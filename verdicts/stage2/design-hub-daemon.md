# Stage 2 Hub daemon design review

Reviewer: Sol. Owner: P5. Lead assignment: `msg_plugin-w_1791847798_b118f0`.
This record reviews the design only. It accepts no implementation, gate, contract amendment, or story completion.

## Round 1: complete design and correction review

Source: `~/botster-sessions/shared/stage2/design-hub-daemon.md`.
Exact reviewed SHA-256: `cd4c35fe6ae61ce65d18fa40d69267c93ceacfecdd4ba60e6eb9771f82b981de`.
Matching owner receipt: `msg_plugin-w_1791848440_2ffdf9`.
The reviewer saved and read the complete source at this hash.

The review uses these complete inputs:

- Stage 2 brief: `93a3be0c7d961d8cee33f1ba7f1be87c23d73fbe167c6d8b2f5f599920530e35`.
- Repo plan: `035b74e2588798d069b17d510a3a9a47f2de324c76b447698982bd858c973617`.
- Hub prior art: `7b975f805ff3af28c6c9198dd38eadfb84ccdcdac2d288b6679db7ffcf1fb766`.
- Approved minimum-Hub pin: `26c2c7959f973d89fc90c05d5789435a5591d59397dfba9636ab49979f04f9f7`.
- Core source at `644f632f194f62ad7fe14e66482ee01083706e30`, including CoreApi and HostDriver event/wake behavior.
- Frozen HC clauses EN-3, EN-4, HS-8, HS-9, WT-1, WT-4, WT-6, and section 11.1.
- Frozen Core LC-11, LC-12, and adoption clauses.
- Steward R-118 and HC A11 candidate 1 at contracts main `66e338a6bed9dde9972a66fe551a8b54afd82b53`.

HC A11 remains a draft. Reading it does not approve or freeze it.
Core A26 remains a separate prerequisite for services. This review grants no permission to start P7 code.
Created-row recovery remains explicitly PENDING under S2-D4. This review supplies no recovery rule.

### Revision history

The reviewer read the initial design at `01c6f46722e6910b21d65b0baf95ae14e93dedae2b3eb19fad1e7f6c480a99db`.
The reviewer sent findings 1 and 2 in `msg_plugin-w_1791848059_a72b1c`.
The reviewer sent findings 3, 4, and 5 in `msg_plugin-w_1791848146_ff28bf`.

The reviewer read correction `c8e330fed00d51d8695fc35aecc7ee83590cfa68a8ea8a74eb23c12472d12855`.
Its full-batch drainage rule was insufficient. Follow-up: `msg_plugin-w_1791848218_68f034`.
Its token refusal path still lost ownership. Follow-up: `msg_plugin-w_1791848297_05fdb4`.

The reviewer also read the complete mutable source at
`f5df08968c7157a824957704214ae271a9234758cab84174f50e4aac8365f48b`.
That read had no matching owner receipt. It does not replace the exact Round 1 source above.
Owner receipts later supplied token-effect corrections, then added and withdrew label-based restore.
The complete Round 1 source includes the token corrections and the withdrawal.

### S2-HD-1 — MEDIUM — CLOSED: drain Core events before waiting

The initial loop could wait with unpolled Core events. Core's `more` and wake report runnable work and worker input.
They do not report every queued event. A partial poll can leave a completion without a future wake.

The first correction continued only after a full batch. CoreApi promises at most the requested count.
A smaller nonempty batch therefore cannot prove that the event queue is empty.

Closure: section 2 sets `events_left` after every nonempty poll. Only an empty poll clears it.
`turn_events` is at least 1. Pending fan-out prevents another poll, and also prevents a wait.
This closes the design path without a diagnostic counter or a new Core API.
Implementation evidence must exercise partial nonempty batches and the final empty poll.

### S2-HD-2 — MEDIUM — CLOSED: keep one reply owner on every dispatch path

The initial design treated every Core request as an asynchronous operation.
It omitted immediate `begin` errors, synchronous reads, CancelResult, and synchronous Attach refusals.
The first token correction also took ownership on admission refusal without returning the token.
That path could trigger the token's fallback and then send a second mapped refusal.

Closure: section 3a distinguishes immediate results from operations that enter the waiter map.
Section 3 returns `Rejected { refused, request, reply }` untouched.
The move-only token fills its reserved slot through one send or one Drop.
IO writes a filled slot once, or frees it after the connection ends.
Stop answers only tokens still held in lanes or waiters.

The owner marks the token dispatched before `begin`, `attach`, or `cancel`.
A dispatched mutation's fallback has `effect: unknown`; a proved pre-dispatch refusal has `effect: none`.
The fallback code is `hub_shutdown` during stop and `internal` otherwise.
This closes the design paths under HC EN-3 and section 11.1.
Implementation evidence must cover rejection, synchronous refusal, panic, disconnect, and stop without duplicate answers.

### S2-HD-3 — MEDIUM — OPEN: fence a rebuilt watch against older work

The initial design submitted a watch rebuild to Bulk. A full lane could reject it and leave a silent gap.
Queued rebuild work also lacked a removal path when a watch ended.

Partial closure: section 4 now uses a dirty bit and a bounded set with one entry per watch.
The dirty mark cannot fail because Bulk is full. Unwatch and disconnect remove the mark and rebuild state.

The design does not define the baseline cut relative to the retained fan-out batch.
It permits a rebuild to use a view that includes later events while the continuation still holds older deltas.
Those deltas can follow the new `snapshot_begin`. HC WT-1 and WT-4 prohibit this order.
The design also does not state how unwatch invalidates watch frames already handed to IO.
Removing the owner map alone does not establish HC WT-6 silence after the unwatch response.

Required change: define view advancement and the baseline cut, or use a cut/generation fence.
Specify how the Hub discards older pending deltas and invalidates queued IO work when the watch ends.
Preserve every delta after the cut, including changes during an incremental baseline.

### S2-HD-4 — MEDIUM — OPEN: wake each stop phase and define lifecycle messages

The initial design did not handle failed adoption or owner failure before it advertised readiness.
It could also wait for IO forever after the owner ended.

Partial closure: section 5 handles refused adoption, failed adoption completion, and early owner exit.
Those paths delete no orphan directory, bind no socket, emit no ready line, release the lock, and exit nonzero.
Owner and IO panic paths retain state outside `catch_unwind` and notify main.
Core Drop remains on the owner thread and leaves workers running under LC-12.
The stop order removes the listener before releasing the lock and emits the final close afterward.

Normal stop step 1 sets `stopping` but does not wake the owner.
A signal can therefore leave the owner waiting in `mio::Poll` with empty lanes and no Core wake.
Normal `OwnerGone` also needs a wake of IO before IO can perform stop step 3.
The IO-panic paragraph specifies an owner wake; the common stop sequence does not.

Section 2 says each thread sends at most one lifecycle message.
The stop sequence requires IO to send StopRequested, a listener-removal acknowledgement, and IoGone.
The stated message rule therefore does not describe the stated stop sequence.

Required change: define persistent stop state and out-of-band wakes for every stop cause and phase.
Define the lifecycle messages, channel bound, and nonblocking send behavior for the complete phase sequence.
Neither thread may depend on room in a request lane to stop.

### S2-HD-5 — MEDIUM — OPEN: account for baseline work in the turn bound

The initial event count did not bound delivery to every watch. The watch map also lacked a global connection bound.

Partial closure: section 4 caps watches by `max_connections × max_watches`.
One retained batch limits ordinary fan-out to `turn_fanout` deliveries per turn.
Control admission occurs before the continuation resumes. No new event batch is polled while one remains pending.

The owner also rebuilds up to `turn_rebuilds` complete watches in that turn.
Each baseline can contain many rows and pages. The stated `turn_fanout` bound does not count these deliveries.
The design does not give a separate work bound for baseline construction or delivery.

Required change: define a bounded continuation for baseline rows/pages, or state the separate finite work bound.
Correct the claim that a turn makes at most `turn_fanout` deliveries if baseline delivery uses another budget.
Keep control admission available between baseline steps. Keep the baseline equal to one cut under WT-1.

### Accepted design scope and evidence limits

The design retains one Core owner, bounded bridge lanes, typed refusals, and result replies.
It uses count budgets instead of a wall-clock scheduler. Terminal bytes bypass the Hub and plugins.
The skeleton excludes services, the plugin runner, client verbs, and product logic.
It reads hub.toml and rejects unknown keys under lead Q3. It binds the socket after adoption under lead Q4.
The 54-byte data-root calculation matches Core's 41-byte maximum InstanceId and `/core/w/` overhead at the named base.
Core A26 can change that calculation; the proposed boundary test must then expose the change.

The reviewer ran no build, test, gate, or reproduction. This record reviews supplied source and contracts only.
The reviewer sent the remaining paths directly to P5 in `msg_plugin-w_1791848533_6eac2b`.
All five original findings remain in this record. Two close and three remain open.

VERDICT: NOT CLEAN (3 open)

## Round 2: corrections and the new durable spawn store

Exact reviewed document SHA-256: `424c8e87a3d2ce9f211062cd9ff56245cb74295cb260e71b159be93eb45ede65`.
Owner receipts: `msg_plugin-w_1791848645_32dc55` and `msg_plugin-w_1791848684_407892`.
The reviewer saved and read the complete document. The review covers the full change from Round 1.

The lead's handoff records the S2-D4 store assignment to P5 and the interface assignment to PC.
The reviewer reads the complete R-120 and R-121 rulings.
R-121 source commit: `906bc705f1c788a7e19653806205e96ee9c426d5`.
R-121 section SHA-256: `e3cc52faa6e86ebbae3531be3b9ed536b4c5e0b69bbaea67f0fb3b513de50579`.

The reviewer also reads the complete `minimum-hub-additions.md` and the complete PC protocol design, Revision 5.
Those inputs establish the affected interface; this record issues no separate verdict on PC's design.
The approved minimum now has 127 ids. The two additions cover durable operation restart and closed operation states.
They do not add the `en_7b_*` family. The review therefore permits an explicit deferral of caller-known operation ids.

### S2-HD-1 — MEDIUM — REOPENED: include runnable baselines in the wait rule

The nonempty-event drainage rule remains correct.
The new baseline continuation introduces another source of retained work.
Owner-loop step 5 still names a dirty watch, but section 4 also queues new WT-1 baselines.
A new baseline can remain incomplete when Core returns an empty event batch and every request lane is empty.
The wait rule does not explicitly keep that runnable continuation active.

Required change: include runnable baseline work in the decision to continue a turn.
If a full outbox prevents baseline progress, define the capacity wake that resumes it.
Do not replace a lost wake with an immediate loop that spins while the client cannot receive frames.

### S2-HD-2 — MEDIUM — remains CLOSED

The returned-token, dispatch, exactly-one-answer, and mutation-effect rules remain present.
The FIFO correction below makes a reservation a count rather than a fixed position.
That change preserves one reserved reply slot and one send or Drop per token.

### S2-HD-3 — MEDIUM — CLOSED

Section 4 applies each event when that event's fan-out starts.
The session view cannot contain later events from the retained batch.
The owner takes a baseline cut only between batches and retains immutable Arc rows for that cut.
Later deltas wait behind the baseline and follow `snapshot_end`.
Overflow removes the old continuation and queues a new baseline without using Bulk capacity.

One FIFO outbox carries frames and responses for each connection.
At unwatch, the owner removes the watch, baseline, and delta queue before it appends the response.
Previously emitted frames precede that response. No later frame can come from the removed watch.
These rules close the specified WT-1, WT-4, and WT-6 paths.
Implementation review must verify the FIFO and cursor behavior, including overflow during a partial baseline.

### S2-HD-4 — MEDIUM — CLOSED for the original owner/IO lifecycle

Section 5 replaces the inconsistent message rule with monotonic atomic phases.
Every phase change wakes both loops outside the request lanes.
The channel has capacity 4 and each of its four message kinds is sent at most once.
The common stop order identifies the actor for each phase and notification.
Owner and IO panic paths enter the same stop state.

The bounded reply flush prevents a client that never reads from holding the stop indefinitely.
The original failed-adoption and Core Drop rules remain intact.
This closes the original finding. The new store thread has a separate lifecycle finding, S2-HD-6.

### S2-HD-5 — MEDIUM — CLOSED

Section 4 now retains a baseline cut and row cursor.
It sends at most `turn_baseline_rows` rows across all baselines per turn, with round-robin selection.
Section 2 states separate delta and baseline budgets.
The design also states that taking a cut copies at most Core `max_sessions` Arc pointers.
This closes the missing baseline-delivery accounting in Round 1.
It establishes count bounds, not a measured byte or time bound.
Implementation review must inspect cut creation, chunk serialization, empty baselines, and destruction within those counts.

### S2-HD-6 — MEDIUM — OPEN: own store jobs through failure and stop

Section 4a adds a store thread with a bounded queue and completion messages to the owner.
Section 5 still joins only the owner and IO threads before it releases the singleton lock.
The store has no stated admission closure, stop notification, panic path, or quiescence acknowledgement.
A queued or active write can therefore continue after main releases `hub.lock`.
That violates the stated RT-4 rule that only the lock holder writes the store.

The design also refuses every job when the queue is full.
That includes `finish_spawn` and retention deletion, which are internal work after the original admission.
The design gives no retained owner or retry path for a refused terminal update.
It also gives no completion capacity rule. A request queue bound alone does not bound unsent completion results.

Required change: define job ownership from admission through durable completion and retirement.
Reserve or otherwise bound completion capacity and its wake path.
Keep required terminal updates after queue refusal, or enter an explicit store fault that preserves uncertainty.
Stop new store admission before shutdown. Establish store quiescence before the singleton lock is released.
Define how store failure wakes main and the owner without blocking their event loops.

### S2-HD-7 — MEDIUM — OPEN: reserve operation identity before the write

Section 4a permits caller-known `operation_id` values and cites EN-7b.
Its record has no normalized request comparison data.
It also does not reserve an identity or record capacity before a store job enters the queue.
Two pending admissions can therefore write separate `record_key` files for one operation id.
Pending writes can exceed a bound that counts only records already loaded into the index.

Required change: count pending admissions in the record bound and reserve each operation identity before store submission.
If caller-known ids remain in scope, define exact-duplicate joining and the `operation_id_in_use` refusal.
Persist the comparison data required after restart, or name its durable owner in PC's layer.
Alternatively, explicitly defer caller-known ids and remove the unsupported EN-7b admission promise.
Do not add minimum ids or implement a new feature solely to close this finding.

### S2-HD-8 — MEDIUM — OPEN: corrupt records cannot prove non-admission

Section 4a excludes an undecodable file from the index, counts it in diagnostics, and leaves it in place.
The same API can then return `not_found` for an operation that the Hub previously admitted.
The design cites EN-7b's retention rule, under which `not_found` within the window proves non-admission.
An unreadable record cannot establish that proof.

Required change: define a typed startup or read failure, or retain a recovery state that preserves this uncertainty.
Do not return `not_found` while a corrupt durable record leaves operation identity unresolved.
Define the disposition of incomplete temporary files separately from committed record files.

### Recovery and acceptance limits

The document leaves its Q5 recovery case pending and cites Core v1 as its base.
R-121 now gives the complete reconciliation table and expressly requires the Core pin that carries A26.
It prohibits an interim Core v1 guess. P5 must carry that prerequisite into the revised design and implementation plan.
This record accepts neither A26's draft nor a guessed restore outcome.

The reviewer sends these paths in `msg_plugin-w_1791848775_a93bf9`.
The scope choice for caller-known ids is in `msg_plugin-w_1791848796_de8554`.
Five original findings remain recorded. Findings 2 through 5 close; finding 1 reopens for new retained work.
The new store adds findings 6, 7, and 8. This round has four open findings.
The reviewer ran no build, test, gate, or reproduction.

VERDICT: NOT CLEAN (4 open)

## Round 3: baseline wakes and store lifecycle corrections

Exact reviewed document SHA-256: `217d75aaadc3f77e852385c6739993dc9f8b9c2c8c9c919f20afa8876a395ba7`.
Matching owner receipt: `msg_plugin-w_1791848977_4326dc`.
The reviewer saved and read the complete document. The review covers its full delta from Round 2.

### S2-HD-1 — MEDIUM — remains OPEN: arm the capacity wake before checking the level

The wait rule now includes runnable baselines and queued store completions.
It distinguishes a blocked baseline from runnable work, so a blocked baseline does not force an immediate loop.
The owner sets `wants_room` after it observes a full outbox.
IO clears that flag and wakes the owner when its writes reduce the frame count to the threshold.

That order still permits a lost wake:

1. The owner observes a full outbox.
2. IO drains the outbox below the threshold while `wants_room` is false.
3. The owner sets `wants_room` and waits.
4. IO has no remaining frame to write. No later write clears the flag or wakes the owner.

Required change: define an arm-then-recheck handshake, or one synchronized level check, before the owner can wait.
The handshake must retain the no-spin rule when capacity is absent.
Implementation evidence must exercise IO drainage between the owner's capacity check and wake registration.

### S2-HD-2 through S2-HD-5 — remain CLOSED

The revision retains token ownership, dispatch effects, between-batch cuts, FIFO unwatch ordering, and counted baselines.
The common stop phases and out-of-band wakes remain present.
The reviewer finds no regression in those original paths.

### S2-HD-6 — MEDIUM — remains OPEN: close the live store on its fault path

The owner is now the only store sender and bounds jobs in flight by `store_queue`.
The completion channel has capacity `store_queue + 1`, including StoreGone.
Each record has one retained terminal slot. Terminal jobs get capacity before creates or deletions.
Runtime publication uses only the durable state after completion.
Normal stop drains store jobs, sends Close, joins the store, and only then publishes OwnerDone.
Owner panic follows the same quiescence sequence. These changes close the earlier normal-stop and capacity paths.

The store-fault paragraph gives a different sequence.
After an Unknown write or a terminal/delete error, the owner sends no more jobs, waits for completions, and joins.
The store remains alive after a write error. Once it drains its jobs, it can wait for another job indefinitely.
The fault sequence does not send Close or close the sender. The join can therefore wait forever.

The paragraph also says every unwritten terminal state leaves a `starting` disk record.
An error after rename can leave the terminal record present. The result is unknown, as the write-error rule states.
The design must not claim a particular disk state after that error.

Required change: send Close or close the sole sender before joining a live store on the fault path.
After StoreGone, retire jobs with unknown outcomes and join without waiting for nonexistent completions.
State that recovery loads either surviving record state after an uncertain write.
Reconcile a surviving `starting` record through R-121 without guessing what reached disk.

### S2-HD-7 — MEDIUM — CLOSED

Caller-known ids are explicitly deferred. The skeleton accepts only Hub-minted operation ids.
`create_spawn(session_id) -> operation_id` reserves the index entry before store queue admission.
Reservations count against `max_operation_records`. Refusal with no free record or job slot changes nothing.
A proved pre-rename failure removes the reservation and returns `effect: none`.
An uncertain write returns `effect: unknown` and starts store-fault shutdown.
The caller receives an id only after the initial write is durable.
This closes the admission and capacity paths within the stated minimum scope.

The PC protocol design read in Round 2 still names the older two-argument API.
The reviewer sends the interface change to PC and its assigned integration reviewer, Sol-B.
Those notices assign no new review work and issue no separate PC verdict.
Implementation must use the same store API on both sides.

### S2-HD-8 — MEDIUM — CLOSED

Startup load now fails on a corrupt record, unknown version, size violation, key/name mismatch, or duplicate operation id.
It also fails if committed records exceed the configured count. It drops no admitted record to meet that count.
The failure is typed `data_root_unusable`, before the listener or ready line.
The load treats unfinished temporary files separately from committed record names.
Runtime reads use the index. They do not skip an unreadable committed file and then claim non-admission.
This closes the specified false-not-found path.

### Recovery and evidence limits

Step 6b now carries the full R-121 table and explicitly waits for the Core pin with A26.
It gives no interim Core v1 recovery rule. The indeterminate rows retain `starting`.
The table is applied again after a later Adopt retry. The retry policy stays outside this skeleton.
Q7 remains a lead item in the document. The approved additions already read in Round 2 remain the acceptance input.

The reviewer sends the two remaining paths to P5 in `msg_plugin-w_1791849035_fd647e`.
PC receives the API notice in `msg_plugin-w_1791849035_b7cc90`.
Sol-B receives the same interface scope in `msg_plugin-w_1791849035_ca3c1e`.
Findings 2 through 5, 7, and 8 close. Findings 1 and 6 remain open. No new finding is added.
The reviewer ran no build, test, gate, or reproduction. This is a design verdict only.

VERDICT: NOT CLEAN (2 open)

## Round 4: SETTLE correction review — CLEAN

Lead ruling: `msg_plugin-w_1791849094_b6a3af`, SETTLE with no split.
The lead limits this round to findings 1 and 6. Findings 2 through 5, 7, and 8 stay closed.
Matching owner receipt: `msg_plugin-w_1791849107_f474a2`.
Exact reviewed document SHA-256: `0d39345fa29f9a449f0d20131716d40e00ab017f5a4135c2877f5609a8270c6d`.

The reviewer saves the source and verifies its hash.
The complete diff from Round 3 changes only the capacity-wake handshake and store close/failure-state rules.
The reviewer reads every changed paragraph within the lead's assigned scope.
This round does not reopen the closed scope or issue a separate protocol review.

### S2-HD-1 — MEDIUM — CLOSED

The owner now arms `wants_room` before it checks `frames` again.
All access to the two atomics uses SeqCst ordering.
If the second check sees room, the owner clears the flag and continues without marking the baseline blocked.
IO reduces the frame count before it checks the flag.
When the count reaches the stated threshold, IO clears the armed flag and wakes the owner.

The ordered handshake closes the Round 3 check-before-arm race.
An IO drain before the arm appears in the owner's second level check.
A threshold drain after the arm produces the wake. A spurious wake is permitted.
A blocked baseline still does not force an immediate loop while capacity is absent.
The partial event-drainage and runnable-baseline rules from earlier rounds remain intact.

Implementation evidence must exercise both orders, including a wake while the owner is still processing its turn.
This closure accepts the design rule, not a tested implementation or a timing claim.

### S2-HD-6 — MEDIUM — CLOSED

The owner now closes the store queue by dropping its sole job sender.
That close uses no queue slot and cannot be refused.
Normal stop first sends retained terminal updates as capacity becomes free, then drops the sender.
The store finishes its admitted queue, sends bounded completions, observes channel closure, and ends.
The owner joins the store before OwnerDone and before main releases the singleton lock.

The store-fault path drops the sender immediately and sends no additional job.
A live store drains its admitted queue and ends. A store that already sent StoreGone has already ended.
The owner joins either path without waiting for an additional request or nonexistent completion.

The revision also removes the false claim that an uncertain terminal write always leaves `starting` on disk.
Recovery loads the surviving old or new record. It reconciles a surviving `starting` record through R-121.
A terminal record remains terminal. An uncertain retention delete can leave the record present or absent.
The Hub claims no particular post-failure disk state.

The reserved completion capacity and one retained terminal slot per record remain the Round 3 rules.
Implementation evidence must cover normal stop, write error, store panic, and owner panic without writes after lock release.
This closure accepts the lifecycle design only.

### Final design scope

All eight findings are closed, including every partial path retained in earlier rounds.
The final review accepts only the daemon design at the exact hash above.
The lead's SETTLE ruling authorizes this focused round; it grants no implementation or gate acceptance.
GS bootstrap, the required Core A26 pin for recovery, and applicable draft-contract freeze prerequisites remain due.
Caller-known operation ids remain deferred. The approved minimum remains 127 ids, as read in Round 2.
Sol-B owns the PC protocol delta. This record does not substitute for that review.

The reviewer ran no build, test, gate, or reproduction.

VERDICT: CLEAN (0 open)
