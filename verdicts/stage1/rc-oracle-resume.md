# PR #269 — R-62 resume oracle

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
