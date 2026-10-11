# PR #269 — R-62 resume oracle

## Source delta after #271 — 2026-10-11

Head: `c2dd1fbfe801f3b9e43256071b01081eb75a0592`.
Base: `e4b6cdf54a7453af182b9e3e44fc08b5f58c7b47`.
Parents: source-CLEAN `1efe84734e6297e1b5c79f123e2161f923dc233d` and the base.
Tree: `91087a9b527bcd242804ea1db106b9928a9dd9b8`.
Package source CLEAN: `324ed659694487d942309c1cc728bc8add48f42a`, `verdicts/rc-real-controls.md`. I read the complete delta round.

I read the complete failed base check and every imported line in the two overlapping files, lib.rs and real.rs.
The check passes both ancestry conditions and condition 1. Conditions 2 and 3 fail on the shared files and index line.
Both pending-file checks pass. The failed result remains failed. This round supplies the required manual delta review.
Independent comparisons confirm that each overlapping file's imported added and deleted lines equal the base patch.
The other eight reviewed own blobs remain byte-identical. Both pending files and mutation configuration equal the new base.
The static merge produces the exact new tree without a conflict. I run no executable base check.

The import adds the already-reviewed route_reports module, argument, control, capability, dispatch arm, and public control assertions.
The control observes reports consumed at the link edge. It changes no returned link byte and holds no Core event.
The import leaves oracle_resume dispatch and the capture layer construction unchanged.
It does not replace the resume control's begin, pump without clock advance, foreign-event refusal, page reads, or release.
The existing public cleanup and isolation assertions remain unchanged. All three source findings stay closed.
No new source finding appears. The code pin remains .36, and no pending identifier leaves through this review.

Check: `~/botster-sessions/gates/botster-core-rc-r69-base-merge-check-c2dd1fbf-20261011-082659.log`.
The owner reports a green final gate at earlier head `1efe8473`. I have not independently matched all of that gate's outcomes.
That earlier gate cannot supply execution at this new head. The final exact-head gate and final body remain pending.
Historical Mac and transcript-fence proof limits remain as recorded below. This source verdict accepts no future gate or import.
I ran no build, test, gate, mutant, reversal, measurement, or executable base check.

VERDICT: CLEAN (0 open; source delta complete; final exact-head gate pending)

## Round 1 — 2026-10-11

Head: `2a0c4922e93f3bb29234a90fe79fc57db0f7b48d`.
Review and gate base: `56fae5826f766b408abca06f99376e0506982bf9`.
Parents: `a52bda8fcb59e3253d718a01bdee919bd7754443` and the review base.
Tree: `a4c143836b6ca9d3766fbc7e47877e5989ec4394`.
Risk: HIGH.

Owner READY: `msg_plugin-w_1791729176_68e708`.
Package verdict: `e61f1329395b3f5efd7279ff3c32dd4a4ea65781`, `verdicts/rc-real-controls.md`, Round 1.
Package scope correction: `1f2ddd5c5af7505a1379ea6f59eec4d5c9d6532a`.
Sol reads the complete package round and its correction. Both commits are published on `origin/stage1/review-rc`.

Sol reads all ten changed paths against the exact review base. The current `origin/v1` is later: `c942593697e856017572977af0405b17de0124e2`.
That merge has parents `56fae5826f766b408abca06f99376e0506982bf9` and `bbd25214835b4648623cb1e5ac3a89ac50f5d260`.
Its tree equals the accepted #262 tree, `401afd40ba6e49c934a03f34ad360f5bfa777456`.
No later #269 import is reviewed here.

### OR-SI-1 / RC-OR-1 — HIGH — The capture layer changes Core event ownership. OPEN.

`capture_layer.rs:122` pumps Core. Line 123 calls the inner `poll_events(usize::MAX)`.
Lines 129–131 move unrelated events into `Shared::held` and count them in `unreported`.
The control can return success with those events held. The transcript receives them through a later layer poll.

This preserves delivery order but changes the event lifecycle:

- `botster-core-host/src/engine.rs:542` removes polled completions from `ops`.
- The same method frees `WriteInput` operation and byte counts at lines 547–557.
- `botster-core-host/src/driver.rs:837` also unparks links and updates the wake after that poll.
- `botster-core-host/src/queue.rs:377` retires route events only from its own `entries`.
- Instance and service retirement use the same queue ownership boundary.

Thus a held completion has already freed Core capacity before the transcript polls it.
A held `Activity` cannot be retired by a later `RouteClosed`, although the transcript has not polled it.
Reporting the held count through `events_posted` does not restore capacity or retirement.

Core section 6.2 requires retirement of unpolled events. EV-5 and AM-4 bind capacity release to polling.
R-66 explicitly rejects a fence that consumes `Activity` before the close and makes the retirement proof vacuous.
R-62 permits a second capture. It does not permit a held queue to replace Core's event lifecycle.
This is a source finding. Sol claims no reproduced retirement failure.

Sol reads the steward's R-69 draft and the lead's plan 23at.
R-69 permits polling for the second capture only with a checked refusal boundary.
Any event other than that capture's own `Completed` must return `ControlError::UnsupportedFor("events_pending")`.
The refusal is never a pass. The control must hold and re-inject no drained event.
On success, the control reads every page and releases the capture.
It restores the public open-capture count and `next_deadline`.
The harness must not model queue retirement or operation admission.

Required change for #269: implement this R-69 form and restore all three removed ST-6b IDs to `core-real-pending.txt`.
Supply the real-tier queued-`Activity` negative proof and the public success cleanup proof.
Plan 23at assigns the transcript fences and five Mac passes per ID to the later contracts and combined pin work.
#269 makes no pin move. Its passing old transcripts do not approve their removal under the new scope.

### OR-SI-2 / RC-OR-2 — MEDIUM — New proofs assert private records. OPEN.

Sol confirms these assertions in the changed source:

- `capture_layer.rs:598`–602 reads the private capture map and asserts session, mark, pages, and `begun`.
- Lines 619–623 assert capture-map membership and an internal mark.
- Lines 688–690 assert the private `changed` set.
- `real/program.rs` adds a test that calls `Programs::wrote` and asserts `Marks::next` and `Programs::output`.
- `real/resume.rs:208` waits on private capture-map state. Line 209 takes the capture ID from that map.

BUILD.md Testing rule 3 forbids tests of helper internals.
These checks do not establish the same behavior through the control boundary or public Core completions.

Required change: prove the output cut, session isolation, recording, and resize/profile refusal through control results and public completions.
Get capture IDs from `Completed`. Use public boundary observations for forwarding, release, order, and cleanup.
Remove assertions of private maps, sets, and counters. Do not expose those records through a new assertion accessor.
Private writes for missing-field fault injection can remain. Assert their control results.
The pure `known_output_history` decision assertions are outside this finding.
The package round also names an inherited `ModelLog::revs` assertion. Sol's independent evidence above covers the new affected paths.

### OR-SI-3 / RC-OR-3 — LOW — The Prior art note omits reasons for custom code. OPEN.

The supplied body states only that old Core has no resume oracle, comparator, or conformance harness.
It gives no ecosystem check or reason for the new custom comparator and capture layer.
Pair-common and BUILD.md rule 0 require a reason for each custom component and prefer maintained libraries.

Required change: identify the existing facilities considered and explain the custom code that the final design requires.
Describe the accepted R-69 capture form. Do not retain a rationale for the rejected held-event form.
Sol does not claim that the old terminal crate contains an equivalent comparator.

### Evidence and acceptance limits

The supplied Linux log names this exact head and base:
`~/botster-sessions/gates/botster-core-stage1-rc-oracle-resume-2a0c4922-pool-20261011-071917-20743.log`.
Its final summary passes all ten steps in 769.7 seconds. The wrapper exits zero after 780 seconds.
It reports 121 mutants: 88 caught, including 29 in stage 2; 33 unviable; zero missed or timeout.
Sol reads the log header and summaries. Sol has not independently matched all raw outcomes and catch reports.
The green summary does not close these source findings.

The package reviewer reports that all three ST-6b IDs pass in that Linux log.
It also reports earlier Mac successes, reversal failures, two default-timeout failures, and a thread-count failure.
Sol has not independently read those Mac and reversal records in this round.
Those earlier records do not prove the future R-69 correction.

The comparator uses libghostty observations. DECRC changes only harness-owned oracle terminals.
R-67 authorizes the testkit's independent program-edge log and its two comparisons.
Sol reads the field cases and negative comparisons. The package reviewer separately verifies the native keys and formatter independence.
Sol makes no independent claim that all pinned encoder imports were checked.
The own diff changes no exclusion. The three pending removals remain unapproved.

Plan section 8 Q8 needs no repeat gate for an unchanged accepted head and base.
A correction or current-base import needs its own exact-head gate. This round accepts no future head.
Sol runs no build, test, gate, mutation, reversal, measurement, or executable base check.

VERDICT: NOT CLEAN (3 open) at 2a0c4922e93f3bb29234a90fe79fc57db0f7b48d

## Round 2 — R-69 correction, 2026-10-11

Head: `eba028f6ec2b4f50dae9c1a3bd73d53b72e15077`.
Base: `c942593697e856017572977af0405b17de0124e2`.
I read the complete control and test correction, the R-69 draft, and the live body.
I read package Round 2 in `48e76c5f94550a77ea98809d3c157c9f0718f48e`.

**OR-SI-1 / RC-OR-1 HIGH — CLOSED within the agreed control-only scope.**
The second capture begins, pumps with the last supplied Now, and polls. It adds no clock step.
Any event other than its own completion returns `UnsupportedFor("events_pending")`. The control holds and re-injects no event.
On success, the control reads every page and releases the capture. A page error also releases the completed capture.
The real negative test sends output, pumps without polling, and requires the refusal.
The real success test compares public captures, pending operations, queued events, and next_deadline before and after two controls.
The three ST-6b identifiers remain pending. Both pending files equal the reviewed base.
The contracts fences, five Mac passes per identifier, and later pin gate remain separate work under plan 23at/23au.

**OR-SI-2 / RC-OR-2 MEDIUM — OPEN narrowly.**
The correction removes direct capture-map, changed-set, program-history, and ModelLog private-state assertions.
The real test obtains its capture identifier from Completed and proves history and cut behavior through the public equal result.
Two assertions still compare `Resumed { mark: 4, ... }`. Shared::resume copies the stored private mark into that value unchanged.
The result still exposes a private record for an assertion. Both reviewers require removal of those two mark assertions.
Page, error, forwarding, and release checks can remain. The real public result must retain the cut and session-isolation proof.

**OR-SI-3 / RC-OR-3 LOW — CLOSED.**
The body compares existing snapshot, terminal-library, formatter, wrapper, page-read, and output-observation facilities.
It gives reasons for the independent comparator, acknowledged output history, and final R-69 capture wrapper.
It retains no justification for the rejected held-event form.

I read the complete supplied Mac reversal record at `884fde19` without executing it.
The earlier cut reversal survives at `1700fa88`. That result remains a failure of the earlier proof.
The later sequence reports five failed reversals, including cut exclusion and every mark zero, through the real public control test.
Record: `~/botster-sessions/gates/botster-core-rc-r69-reversals-mac-884fde19-20261011-074628.log`.
This record is not an exact-head full Mac pass at eba028f6.
The eba028f6 gate summary reports all ten steps PASS in 833.7 seconds, with 86 caught and 34 unviable mutants.
It reports zero missed or timeout. I have not independently matched every outcome and catch report.
This source round accepts no final gate, transcript removal, or future base import.

VERDICT: NOT CLEAN (1 open)

## Round 3 — private-mark correction, 2026-10-11

Head: `c3713dba91e38785eeadeb36444a86f51963a622`.
Base: `c942593697e856017572977af0405b17de0124e2`.
Parent: `eba028f6ec2b4f50dae9c1a3bd73d53b72e15077`.
Tree: `e84837003946c67df36ad39f517000e39f11d7db`.
Package source CLEAN: `48e76c5f94550a77ea98809d3c157c9f0718f48e`. I read its complete Round 3.

I read the complete one-file correction: 13 additions and 19 deletions in capture_layer.rs tests.
Both stored-mark assertions are removed. The helper keeps only returned pages and propagates errors.
The real public equal result retains the cut and session-isolation proof. OR-SI-2 / RC-OR-2 closes.
OR-SI-1 / RC-OR-1 and OR-SI-3 / RC-OR-3 remain closed. Zero source findings remain.

Independent comparisons confirm the six other own blobs equal the prior full source review.
Cargo.lock, mutation configuration, and both pending files equal this reviewed base.
The inherited #262 import adds its accepted enforcement and notes. This PR changes no exclusion or excluded mutant set.

This verdict completes source review at this exact head and base. It does not establish a gate result for c3713dba.
The exact-head gate, any current-base import review, and the lead's merge decision remain pending.
The earlier gate remains earlier-head evidence. No pending identifier leaves through this verdict.
I ran no build, test, gate, mutation, reversal, measurement, or executable merge-check job.

VERDICT: CLEAN (0 open)

### Q7 source transfer — 2026-10-11

New head: `1efe84734e6297e1b5c79f123e2161f923dc233d`.
New base: `010f2a49952f9b77581f304dbaa41006b115d40b`.
Parents: `c3713dba91e38785eeadeb36444a86f51963a622` and the new base.
Tree: `cbdc7b1ba16c7b81c2af964fabb5b23b2026477f`.
I read the complete supplied base check at `gates/botster-core-rc-r69-base-merge-check-1efe8473-20261011-081147.log`.
Every condition passes. The own diff is 92994 bytes, SHA256 `cc969502a0619ec0345f955a75ca4c60d2d476b292255bea9f6bc79920ac1981`.
Independent reads confirm the exact parents, ten disjoint base paths, identical complete own diffs, and unchanged transferred blobs.
I read the published package transfer at `79f6f8c108c2c2f478559e92e505c5f8411db3cf`.
Under Q7, source CLEAN transfers without a new delta round. Every source finding remains closed.
The final exact-head gate remains due. This transfer accepts no future gate or pending removal.
The new base is the #270 merge. Its exact parents and tree preserve the reviewed #270 source.
I have not received or read #270's final new-head gate. This merge check makes no independent gate-acceptance claim for #270.
I ran no execution job.

VERDICT: CLEAN (0 open)
