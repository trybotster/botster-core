# Integration review: #233 program controls

## Source inspection before Round 1 — 2026-10-10

Inspected head: `e75ba5f71c6187697383fd9b7f298548c5ac8008`.
Base: `b028a269383b8d2af9ac349b395765a47f1b8f1d`.
Plan: revision 23ae, `b895ad493b9919dd63a9e11a596ba5fd210f1ea5`.
Contracts: contracts-v0.1.27, `b161db87ca82873b4f6b21d8433a69f086abb7cf`.

The implementer supplied this head for early source inspection. The implementation is not READY.
The exact-head green gate and package verdict remain pending. This inspection completes no review round and grants no CLEAN verdict.

I read all five changed files, the P3 mapping, the PC brief, and both PC handoffs.
I read the contracts probe control loop and the shared process wait and read implementations.
The real harness serves the five program controls together with the merged P5 controls.
The plain harness serves no control. Both harnesses supply their own probe socket through probe_env.
The request and report types come from the pinned contracts crate. The harness does not write PTY bytes.
The observer wake exception and its no_spurious_wakes limit remain in the PR body and P3 mapping.
The temporary accept exception names the bounded, non-blocking loop and P6's replacement obligation.

The pending-list diff removes 16 IDs. It retains all six held-write IDs from the PC audit.
It also retains ev_5_full_queue_defers_transition_not_output. The handoffs' claim of 17 removals is incorrect for this head.
The three removed minimum IDs are ad_3_adopted_equals_started, tm_3_due_deadline_fires_in_pump, and tm_3_next_deadline_is_host_armed.
No green exact-head evidence establishes these removals yet.
I read all three minimum transcripts at the pinned contracts commit.
ad_3 retains its Activity(Output) fence before the terminal and screen checks, and its await_pty fence after adopted input.
The two tm_3 transcripts use the existing harness clock controls. The program control does not advance a clock.

### SI-1 HIGH — A request write has no deadline and holds the report lock

Evidence: real/program.rs:198–212 and 297–299. The accepted socket is blocking.
request() holds Hub.state while requests.write_all() writes the complete request line.
The acknowledgement deadline starts only after write_all returns.
A peer that stops reading can block this write without a deadline.
The report reader needs the same lock to update input and acknowledgements. Programs::drop needs that lock to start shutdown.
The contracts probe sends reports from the same event loop that reads requests.
With both socket directions under backpressure, the held lock also prevents the harness from draining reports.
The new connection and acknowledgement deadlines therefore do not bound the complete request.

Required change: bound request I/O with the shared cleanup deadline. Release Hub.state during blocking I/O.
Preserve request order and acknowledgement association. Keep shutdown possible during a failed or blocked write.
Supply a socket-boundary proof that a peer which does not read cannot hang the request or cleanup.
Status: OPEN. Source evidence only; I ran no reproduction job.

### SI-2 MEDIUM — Exit watchers survive harness cleanup

Evidence: real/program.rs:168–173, 237–253, and 408–422.
connected() drops each exit watcher's JoinHandle. Programs owns only its accept thread.
watch_exit() re-arms await_end after every cleanup deadline. It never checks Hub.state.closing.
The request tests watch std::process::id(), which remains live after Programs::drop.
Those watchers retain their Hub references and continue until the test process exits.
The Drop comment says the watcher ends at its next bound, but the loop does not implement that condition.
The shared await_end documentation also warns that a reused pid can select another process.

Required change: own every watcher. Stop every watcher when the harness closes, within the existing cleanup bound.
Join every watcher and release its Hub reference. Do not reap a production-owned worker.
Supply a cleanup proof with a watched process that remains live after the harness closes.
Status: OPEN. Source evidence only; I ran no reproduction job.

### SI-3 MEDIUM — Cached probe state bypasses process identity

Evidence: real/program.rs:65–70, 154–181, 198–201, and 231–233.
State keys probes, watched, and exited by pid. connected() checks start_time only when watched.insert(pid) succeeds.
The next call with that pid skips the identity check, even when its ProcessIdentity has a different start_time.
The loop also returns a cached probe before it checks exited.
Thus a cached probe can override a failed identity check. A reused pid can also select state from the previous worker.
The probe hello supplies only its parent's pid. The hello alone cannot establish the recorded worker's start_time.
This contradicts the stated R-46 process identity check and can apply a control to another recorded worker identity.

Required change: bind cached probes and exit watches to the complete recorded ProcessIdentity.
Reject mismatched identities before returning a probe. Do not reuse logs, acknowledgements, or exit state across worker identities.
Supply a boundary proof that an existing connection does not make a mismatched start_time valid.
Status: OPEN. Source evidence only; I ran no reproduction job.

I sent all three source findings to the implementer and package reviewer.
I changed no product code. I ran no build, test, gate, mutation job, merge-check job, or reversal job.

## Correction source inspection before Round 1 — 2026-10-10

Inspected head: `ebac3c692bcd317b6e4f53fb98c976cafc24edf0`.
Correction commit: `375a1bb91ae72573f73e2ef4ee1a28aa825dcc1f`.
Merged base: `0a564513`, which includes reviewed #237 and #234.
The implementer reports a running gate and will send READY later.
No completed exact-head green gate or same-head package verdict is supplied.
This source inspection completes no round and grants no CLEAN verdict.

I read the complete correction delta and its callers, report reader, cleanup, and socket proofs.
Only real/program.rs changes in the correction commit after the previous inspected head.
The automatic Git merge tree equals the published head tree: `6bafaa011c2ead99f22f5cc94b4e4937b1d8a592`.
This is a comparison of source objects. I ran no executable merge-check job.
The own diff over the merged base still changes the five PC paths.
The pending-list removal remains sixteen IDs. The six held-write IDs and ev_5_full_queue remain pending.

### SI-1 HIGH — Partial source correction; proof and deadline remain open

request() now takes an Arc of the request socket and the acknowledgement target under the lock.
It releases the lock before writing and takes the lock again for the acknowledgement wait.
The accepted socket is nonblocking. send_line waits for OUT with the shared cleanup deadline after WouldBlock or Interrupted.
The write and acknowledgement wait use one deadline. The report reader keeps its existing Bounded line reader.
This removes the reported lock-held blocking write.
The serialized public control dispatch retains request and acknowledgement order.

Two points still prevent closure of SI-1.
First, send_line checks expiry only after WouldBlock or Interrupted.
Its positive-write path can start another write after the deadline has passed.
The shared Bounded reader checks expiry before each new read, including reads after positive progress.
Required change: check the same deadline before each request write. Keep the existing timeout failure.

Second, reports_are_taken_while_a_request_waits_for_room sends its Input report immediately after it spawns the request thread.
The report can finish before the request starts. The proof can therefore pass with the old lock-held write.
Its one-megabyte request proves no ordering between the write and report processing by itself.
Required change: observe request bytes at the real socket before sending Input.
Then leave the remaining request unread until the report arrives.
Keep the exact input-log and acknowledgement assertions. Use the existing cleanup bound for the fence.
Also retain a socket-boundary proof that a peer which never drains the request produces Bad at the cleanup bound.
No private accessor is required. Status: OPEN, pending the source and proof corrections.

### SI-2 MEDIUM — Closed in source

The correction removes watch_exit and all exit-watcher thread creation.
connected() checks the process identity when it waits and uses the existing cleanup bound.
Programs owns its accept thread. The accept thread owns and joins all report reader threads.
Drop starts shutdown under the state lock, ends the connection sockets, and joins the accept thread.
There is no detached watcher or watcher-held Hub reference left to stop.
No code reaps a production-owned worker.
The exited-before-connect proof uses an OwnedChild and confirms the child's exit before the request.
This source change closes the detached-watcher finding. Exact-head evidence remains pending.

### SI-3 MEDIUM — Closed in source

State.probes now uses the complete ProcessIdentity as its key. The pid-only watched and exited sets are gone.
The report reader records the parent's pid and observed start time together at registration.
Request, acknowledgement, and input lookups all use that same complete identity.
The new socket proof connects a probe, then requests input with the same pid and another start time.
It requires Bad and cannot select the earlier probe.
This source change closes the reported cross-identity cache lookup. Exact-head evidence remains pending.

I sent the remaining SI-1 points to both PC agents before READY.
I changed no product code and ran no build, test, gate, mutation, measurement, or reversal job.

## SI-1 follow-up source inspection before Round 1 — 2026-10-10

Inspected head: `686300e2b1e6cbaded29761dc8963483b4ac03bc`.
Previous inspected head: `ebac3c692bcd317b6e4f53fb98c976cafc24edf0`.
Only real/program.rs changes: 33 insertions and four deletions.
I read the complete follow-up delta.

send_line now checks the shared deadline at the start of every loop pass, before each write.
The positive-progress path cannot start another write after an observed expiry.
The existing nonblocking socket, OUT wait, released state lock, and acknowledgement deadline remain.

FakeProbe::request_started waits for socket readiness with IN and the existing cleanup bound. It consumes no request bytes.
The report proof calls that fence before sending Input and leaves the request unread until the report arrives.
The report can no longer complete before the harness starts the request.
The proof retains the exact input bytes, complete request, acknowledgement, and successful control result.
The new never-draining-peer proof leaves a one-megabyte request unread and requires Bad from the write deadline path.
These assertions observe socket and control results. They expose no private product state.
The implementer reports that the lock-held-write reversal fails locally. Its retained evidence is still to be supplied with READY.

SI-1 HIGH is CLOSED in source. SI-2 and SI-3 retain their preceding source closures.
All three findings now close in source. This does not substitute for the final gate or same-head package verdict.
The implementer reports a running gate and will send READY later.
This inspection completes no round and grants no CLEAN verdict.
I changed no product code and ran no build, test, gate, mutation, measurement, or reversal job.
