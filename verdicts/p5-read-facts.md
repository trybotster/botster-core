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

## Round 4 — complete package review CLEAN

Head: `aeffbde6b393eb583f4363bd812fb2d64af0a057`.
Tree: `fc98d5af776d3aca16f7e912c033500b1ddbf7f6`.
Source base: `3e655eff46d36f8c337339a1a17d401c8c662594`.
Gate comparison base: `3e4b3ab8172f1a04467a7969833708606a0f8580`.
Previous inspected head: `db97736b173d32ab78377a8f5ec2c6d083636bb8`.
Combined READY receipt: `msg_plugin-w_1791844299_6d3616`.
The combined READY meets the lead's one-head condition and ends the inspection hold for this review.
The reviewer reads every correction path, all new tests, the complete body, and applicable original source and merge records.
The complete nineteen-path correction has 770 additions and 67 deletions. Its whitespace check passes.
Full-index correction SHA-256: `dc2829a07b7c3120e5e2abf323c28999f7a46a625e3a35ccc52bbef489baee1c`.
The reviewer verifies that the published P5 branch names this exact head.

### Findings and source conclusions

RF-PC-1 / R320-SI-1 LOW remains closed. Topic files, indexes, references, and the final prose wrap remain correct.
The new design prose wraps within 120 columns. No runtime change belongs to the earlier topic moves.

RF-PC-2 / R320-SI-2 MEDIUM closes. Facts::read now includes current byte and count overflow in gap.
The requested cursor cannot suppress that signal. The first-input test covers pending and completed overflow without an older entry.
It also checks same-instance incremental reads at the pending and final cursors. Normal bounded reads retain their prior gap rules.

RF-PC-3 / R320-SI-3 MEDIUM closes. The host posts completion Activity only when its session still owns the route.
The worker's status update remains unchanged and occurs after a client transaction ends, including a transaction whose route already closed.
The new host test observes RouteClosed, then feeds completion for that route and an unknown route. Both produce no Activity.
The live-route test uses an attached route and preserves the expected event and revision.
The owner also applies this guard to ClientInput, because DetachLocal can close a route without the worker.
That arm still updates the client revision. Its test proves no Activity for the retired or unknown route.
The observation test now uses a live route and preserves its existing values. No key can be recreated through either corrected arm.

RF-PC-4 / R320-SI-4 MEDIUM closes. ordinal_decoder calls the production session::next_ordinal decoder.
receipt_decoder calls record_of_client_frame, which uses the same of_decoded mapping as the production route receiver.
The mapping covers decoded, refused, invalid, and ignored frames. It preserves the previous receipt behavior.
The record constructor remains the same constructor that production uses. There is no alternate decoder for tests.
Both harnesses cap stable property cases at 256 and use the existing pinned-seed mechanism.
The ordinal properties check parsing, saturation, and a written counter's next value. The absent-row case remains explicit.
The receipt properties check pending state, null PTY count, payload limits, truncation, and JSON round trips.
Both new binaries invoke the real built_in_test macro. No handwritten root test is added.
The manifest and lock changes add only the existing workspace bolero dependency to these two crates.
The existing HARNESSES table registers both decoders. No new xtask check or check family is introduced.
Both properties and root tests pass in the default gate tier; both new fuzz runs actually complete.

RF-PC-5 / R320-SI-5 MEDIUM closes within the lead's explicit R320-Q4 scope.
The host and tests now use one core-link formula for the private frame bound.
It keeps the larger of the previous payload term and the complete facts term, adds 1 MiB, and applies the existing cap.
The facts term includes input_events_bytes, four protected entries with escaped source tags, and the client tail as hex.
The source uses saturating arithmetic for the configurable sum. The u32 terms fit the intermediate u64 products.
The protected-entry measurement covers maximal numbers, all current kinds, escaped tags, and the largest status update.
The large-tail test validates Sol's limits, sends ReadFacts through the worker link, and observes Done with the complete tail.
The host test ties the actual engine bound to the same formula. Exact-value tests cover each term and cap behavior.
Default limits retain the previous 97 MiB frame bound. The change adds no public configuration restriction.

The fixed-cap case remains unchanged by explicit lead receipt `msg_plugin-w_1791840875_a81170`.
The proposed A26 ceilings remain a separate amendment: tail 64 MiB, record bytes 256 MiB, and tag bytes 4 KiB.
The body states that A26 is on hold and that valid over-cap configurations remain possible before an amendment.
This CLEAN accepts only the ruled bound repair and test. It grants no A26 acceptance or new open-time refusal.

RF-PC-6 / R320-SI-6 MEDIUM closes under full R-117. Facts now protects newest and the current client record separately.
Each protected identity retains its status update. The identities coincide when the newest record is a client record.
The client-specific set is empty before the first client input. A later client record replaces that protection.
Retention drops only unprotected entries, oldest first. The protected set can exceed either bound, and overflow always sets gap.
The ruled test retains both completed pairs after later host writes with input_events_kept=2.
It checks count overflow for incremental readers, including a reader at the final cursor.
The pending-client test preserves the client record through later host input and its eventual completion.
It then verifies that a later client input takes over protection.

The reviewer accepts the owner's fit choice. A new host record uses the space left by the unchanged client pair.
For a pending client record, the calculation reserves its largest status update. For a completed pair, it counts the actual update.
The calculation also includes the two additional list separators. The new host record keeps the longest payload prefix that fits.
It becomes minimal when no prefix fits. An already published client record is never changed for a later host input.
This preserves incremental-reader consistency and the contract's immutable original record. R-117 permits protected-set overflow.
The exact-room test covers pending and completed client pairs and proves that the payload fills its room within one byte.
All five prior client_pair_len misses are caught by that test in the current gate. No equivalent-mutant claim replaces the proof.

### Gate and mutation evidence

Gate: `~/botster-sessions/gates/botster-core-stage1-p5-read-facts-aeffbde6-pool-20261012-151657-16069.log`.
Verified SHA-256: `77701cf04ba815c7846b472fe87f6c0c8bf09ee4be466404a1d499c73dfb4d59`.
The log names exact aeffbde6 and comparison base 3e4b3ab8. The source merge base remains 3e655eff.
All ten steps PASS. CI total is 787.7 seconds. Job and wrapper exit zero after 798 seconds, with one second queued.
Default runs 2167 tests, all passing, in 17.049 seconds. Slow runs 694 tests, all passing, in 44.569 seconds.
Strict reports 299 passed, zero failed, 393 ignored; it runs 282 of 299 pending ids, with 17 real-only and four held.
The normal conformance summary also reports 299 passed and zero failed. Slow coverage checks 1100 slow-only tests.
The slow step verifies seven named real proofs and three held budget ids at seeds 0-31.

Mutation stage 1 tests 239 mutants: 200 caught, 39 unviable, zero missed, zero timeout.
All 200 unique raw catches match the 200 citations. Every cited test has an ordinary default PASS in this exact gate.
The unmutated baseline passes. Stage 2 does not run because stage 1 misses no mutant. No FLAKY line occurs.
The 39 unviable outcomes earn no catch credit. No new exclusion or retry authority is added.
The earlier 161b7bfd gate is RED: both stages miss the same five client_pair_len mutants.
Its verified SHA-256 is `5020bd5bb4b14e7b5ba08048258054750e16c43f134d1fb577d34f3ff222847e`.
The final gate catches those exact five identities after the committed test repair. No same-head retry is claimed.

Actual completed fuzz runs, each configured for 60 seconds and reporting 61 seconds:

- ordinal_decoder: 40,198,484 cases.
- link_decoder: 6,567,243 cases.
- msg_decoder: 3,884,506 cases.
- receipt_decoder: 4,933,741 cases.

No fuzz run reports a crash. The complete fuzz step passes after 382.9 seconds, including its build work.

All eighteen own ST-7 removals pass in the default gate. Seventeen also pass in the real slow suite.
The cost id remains real-pending, and facts_after_link_loss remains core-pending. No minimum id changes.
All ten conformance blobs, mutation configuration, and known-flaky data remain byte-identical to db97736b.
The gate reports 30 unrelated passing pending-real ids. This review grants no additional status removal from those results.

### Local and reversal records

The reviewer reads the retained local producer and its complete relevant output and exits. No producer is executed by the reviewer.
checks-aeffbde6 records exact head, clean status, and cargo clean exit zero at 22:12:08 UTC.
It unsets RUSTUP_TOOLCHAIN and sets both concurrency caps to four. Fmt, clippy, unit tests, strict, and lists exit zero.
The host passes 387 unit tests and the worker core passes 273. Both new props binaries pass their three tests.
Strict explicitly uses seeds 0-31 and reports 299 passed, zero failed. These local results remain supplementary under pair-common.
Checks SHA-256: `8b39a65c827867972af34878c971bca225afe4443a0e6f55d45ea23b6f0c0b0b`.
Taint SHA-256: `4c6baec79643dd82cfaa0f7ccd66695b58f450a6e7892ebad3beb5e55acd7193`; its only step passes.

The 88ad881f frame reversal records head, target, clean exit zero, and the exact removal of the facts max term.
The actual large-tail read test fails at its missing Done assertion with exit100. Restoration records dirty=0.
Reversal SHA-256: `a3449bce9231b7967593d891b3d0650e479ef421b770af640701277ae6bf335c`.
The aeffbde6 pair-room reversal records exact head, clean exit zero, and five individual patches.
Each patch fails the exact-room test with exit100. The record restores the source with dirty=0.
Reversal SHA-256: `21e014d27de65386372f7cf94ed9e790409e063b0998840f71acca69fd0f0a42`.
The pair-room output is filtered; the frame output is complete. Neither record runs a fresh restored suite.
The subsequent clean local checks and exact restored-head pool gate supply the positive evidence.
These reversals remain local support. They are not relabeled as pool executions or final train evidence.

### Body and disposition

The reviewer reads the complete body and all changes from the earlier body.
Final body SHA-256: `04ac1083666433895015942c9ea80b8b73e02b005c1e9fd2eeabe20a3177403e`.
Receipt `msg_plugin-w_1791844445_eb00ab` corrects the memory sentence without changing source or evidence scope.
The body now states the protected-set exception beside its list-bound claim. This prose issue closes.
The body preserves prior RED gates, the DP-6 merge repair, exact gate bases, and supplementary local evidence.
Its limit sweep includes every facts-size term and the accepted over-cap limitation. Prior art and HIGH tier remain explicit.
The package review is complete on the exact source, body, and supplied READY gate. All six numbered findings close, LOW included.
The matching complete integration verdict remains due. This package CLEAN does not substitute for that record.
Current-base import and applicable deltas, the exact merging train gate, final receipts, and lead Q7 remain due.
The proposed A26 amendment remains separately owned and on hold. No later head, pin, retry, status removal, or merge is accepted.
No reviewer build, test, gate, mutation, reversal, reproduction, or executable base-check job runs.
Wait for the doorbell. Never poll.

VERDICT: CLEAN (0 open; complete #320 package review on aeffbde6, within the lead's R320-Q4 scope).

## Matching complete integration receipt

Receipt: `msg_plugin-w_1791844720_2acc57`.
Integration verdict: `d94618f367ce487a78c0f1e510230d110c993575`, `verdicts/stage1/p5-read-facts.md`, complete Round 4.
The reviewer reads the complete applicable integration verdict and verifies its publication on the remote integration branch.
It is CLEAN on exact head aeffbde6, tree fc98d5af, source base 3e655eff, and gate comparison base 3e4b3ab8.
It reads the complete matching package verdict at `04e0e8102717c022655b52b594c602e6eab457e6`.
All six findings close, including LOW. Both reviews accept the fitting choice and corrected body hash 04ac1083.
The gate, mutation citations, all four completed fuzz runs, local and reversal limits, and unchanged conformance scope agree.
Both records retain the lead's R320-Q4 bound-plus-test scope. The over-cap case stays unchanged, and A26 remains separate and on hold.
No source, body, or evidence finding remains in the reviewed package scope.
Current-base import and applicable deltas, the exact merging train gate, final receipts, and lead Q7 remain due.
This matching receipt grants no later head, amendment, pin, extra pending removal, retry, or completed merge.
No reviewer execution job runs. Wait for the doorbell. Never poll.

VERDICT: CLEAN (0 open; matching complete source reviews recorded, landing requirements remain due).

## Round 5 — approved current-base import CLEAN

Merged head: `48ff7a038d7c466776ae114acf32fd4f2f6d22da`.
Tree: `5b0908bfefc6bf3e79dbd588d5a5fa5ef6beb550`.
Previously reviewed head: `aeffbde6b393eb583f4363bd812fb2d64af0a057`.
Imported v1 and new source base: `644f632f194f62ad7fe14e66482ee01083706e30`.
Sole merge base: `3e655eff46d36f8c337339a1a17d401c8c662594`.
Owner receipt: `msg_plugin-w_1791845122_526e45`. The shared lead handoff confirms the import order after both complete CLEANs.
The merge has exactly the reviewed head and approved v1 as parents. No later correction enters this head.
The reviewer verifies that both remote v1 and the P5 branch name the supplied revisions.
Import full-index diff SHA-256: `286cedef7f627371cbf926623eef2ed5a397cbd1925b8b2f14ab65c3b3bd5548`.
Own diff against the imported base SHA-256: `abfb214921c1f1aa1be3a5290c95735fff9124079f4c19780007cb39932b5856`.
The whitespace check passes.

### Own-source preservation and resolutions

The reviewer compares all 47 own paths against their reviewed source-base patches. Eleven overlap the moved base.
Forty-six own added/removed line sequences remain exact, apart from hunk positions.
The remaining path is the worker design index, whose header already exists in the imported base.
Its own read-facts entry and paste rule remain exact. The merged index retains both topics in file-name order.
The reviewer reads the complete remerge resolution diff. Only two additive conflicts require a resolution.
The design index retains read-facts and the imported read-reply-order topic.
The worker test module list retains exit_watch, read_facts, and routes, in order. No test module disappears.
Every changed non-own blob equals the approved base. The import carries no additional own change.

All host production and test blobs, core-link facts/frame code, facts retention, input, receipt, route, and model blobs remain exact.
Both own decoder harnesses, root macros, source exports, dependency edges, and tests remain unchanged.
The imported lockfile changes contract sources to .46 and adds the base's testkit bolero edge.
The own host and worker bolero edges remain exact against that imported lockfile.
The merged HARNESSES table retains both own decoders beside the imported FrameEnds and sock_diag decoders.
The base's held-full accounting remains intact outside those own registrations. No new check family enters this merge.

The shared worker and testkit files import the accepted exit-watch handling and unbounded drain implementation.
Their own ReadFacts launch, clock, operation, receipt, tail, and completion hooks remain preserved.
The worker still sends waiting output observations before a read's Done through its ordered report path.
The new read-reply-order test coexists with the unchanged own read tests and ordinal-bearing launch fixture.
The testkit keeps records_cost and its measurement decoder beside the new exit-watch fields and actions.
The new ExitFailure binding does not replace or remove measurement. The common real-worker launch retains cursor_ordinal=1.
The imported FIFO and exit-tail fixtures remain base changes and do not change the own frame-size or retention proofs.
The R-116 and R-117 repairs, both Activity guards, and the accepted R320-Q4 frame-bound repair remain exact.
None of the six closed findings reopens.

### Contracts and status preservation

The new pin and source match imported v1: contracts-v0.1.46 at `7e97064c0dc8fe8311c86aea4bac1e4b3663d6b1`.
The reviewer compares the old .44 source `99e5983befde9a96ca4cdb3faea0f7f9a4b930ea` with this exact contracts source.
All twenty ST-7 transcripts, the facts type file, and the limits file remain byte-identical.
The route codec has no source change between those revisions. The .46 fence exception list contains no ST-7 entry.
The import introduces no new ST-7 expected value, limit formula, decoder interpretation, or required control.
The older .44 gate retains its exact old pin and head. It is not .46 execution evidence.

All nine conformance text sets satisfy the three-way formula against the sole merge base.
Every imported removal stays removed, and every imported addition remains present.
Final counts: ledger 692, core-pending 367, real-pending 38, minimum 69, held four, real-only 26, and seven proof rows.
The own eighteen ST-7 removals and one real-pending cost entry remain the only own status changes.
No new minimum removal, pending-real removal, mutation exclusion, or retry authority is introduced by #320's import.
All other status blobs, mutation configuration, and known-flaky data retain the approved base.

### Supplied checks and evidence limits

The reviewer reads checks.sh and retained records. The reviewer executes no producer.
checks-48ff7a03.log records exact merged head, clean status, and cargo clean exit zero at 22:39:31 UTC.
The producer sets both concurrency caps to four and unsets RUSTUP_TOOLCHAIN for cargo.
Fmt, clippy of nine crates, unit tests, strict conformance, and lists each record exit zero.
Host unit tests pass 387, testkit passes 480, and worker core passes 282. Both own props binaries pass all three tests.
Strict explicitly uses BOTSTER_SEEDS=0-31. It reports 299 passed, zero failed, and 394 ignored in 5.31 seconds.
It runs 280 of 297 pending ids, with 17 real-only not run and four held. Lists reports the counts above.
These Mac results remain supplementary under pair-common. They do not prove fresh real-tier, slow, mutation, or fuzz results.
Checks SHA-256: `2ec4bb242c5987096c16880f83bb8cb34bc14f7c42ce78dbf9bc01953eb4747a`.

The locked workspace check record finishes successfully in 8.15 seconds. The owner receipt reports its command and exit zero.
The retained record has no independent head, clean, command, or numeric exit marker. This review keeps that provenance limit.
Its warnings concern CONTROLS, Answer/Withhold, and install in two unchanged testkit files, without the slow feature.
The same blobs equal the imported base. Clippy with the slow feature and warnings denied records exit zero.
Locked-check SHA-256: `d2f995b1132102650b24502637623546c5c42ef72b69f2572112628ea1faaa05`.
Taint SHA-256: `5d2886a97c6abc1a07ece7f6e40f40e9125a0f624596f13a574ce7e3e1321fc4`; its sole step passes in 26.5 seconds.
No fresh full gate, mutation run, or fuzz run is supplied for this merged head.
The accepted aeff gate remains applicable evidence for the unchanged own implementations, with its original .44 composition.
The lead assigns the merged package to train 11; the exact train gate supplies the merging evidence.

### Body and disposition

The reviewer reads the complete current body and its complete delta from the corrected Round 4 body.
Body SHA-256: `d4e137ba0c3e46df85a35b2d6b616c5e090b6942d99a565efc980a66f5a7ebbf`.
The body identifies the approved import, additive conflicts, local checks, and absence of a separate merged-head gate.
It preserves earlier RED history, exact parent gate scope, evidence limits, protected-set exception, and the A26 hold.
Zero source, body, or import findings remain, including LOW. All six finding closures remain valid.
The matching complete integration delta remains due. Applicable train composition review, exact train gate, final receipts, and Q7 remain due.
No final train, future pin, A26 acceptance, additional status removal, specific retry, or completed merge is granted here.
No reviewer build, test, gate, mutation, reproduction, or executable base-check job runs.
Wait for the doorbell. Never poll.

VERDICT: CLEAN (0 open; #320 source, body, and approved current-base import on 48ff7a03).

## Matching complete Round 5 integration receipt

Receipt: `msg_plugin-w_1791845513_130162`.
Integration verdict: `eadea000c4c0494d44c5a9963a2c9ff36c5f945d`, `verdicts/stage1/p5-read-facts.md`, complete Round 5.
The reviewer reads the complete applicable verdict and verifies publication on the remote integration branch.
It matches package `39bbf0f9d277673587c40d6047f696ff3112aa91` on exact 48ff7a03, tree 5b0908bf, and imported base 644f632f.
Both reviews preserve all six closures, LOW included, and accept only the two additive merge resolutions.
The own-path comparisons, unchanged facts interfaces, .46 transcript scope, status formulas, body, and local evidence limits agree.
Both records retain the aeff gate at its actual .44 head and require the later exact merging train gate.
Both records keep A26 separate and on hold, with the explicit R320-Q4 exception unchanged.
No source, import, or body finding remains in the reviewed package scope.
Train 11 composition review, its exact gate, the single final receipt, and lead Q7 remain due.
No later source, amendment, pin, extra pending removal, retry, or completed merge is accepted.
No reviewer execution job runs. Wait for the doorbell. Never poll.

VERDICT: CLEAN (0 open; matching complete import reviews recorded, train acceptance remains due).
