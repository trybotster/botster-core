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
