# Integration review — #299, contracts .41 to .43

## Round 1 — source, pin evidence, and repair history

PR: https://github.com/trybotster/botster-core/pull/299. Risk HIGH: pin, shared testkit, and CI job.
First READY: `msg_plugin-w_1791773303_1ebb01`.
Initial head: `18334c6fe59d11744ba57bc1e7da63c7303300b2`, tree `d17c62a18f419219469dd0af3105996209c85339`.
Actual imported base: `b3cdfffbba353bf9cfc34f7d7f9b8daccc753035`.
Gate comparison reference: `bfef88653bb59ad8d3cd38da1e206f7aba64ce92`, which is not an ancestor of the reviewed heads.
Sol independently verifies the actual merge base is b3cdfffb. The SLOT import is still due.
The initial own diff has nine files, 147 additions, and 31 deletions.
Sol reads that complete diff, all later source repairs below, the full published body and its deltas,
the pinned R-86 trait, changed control definitions, BUILD.md delta, and relevant contracts driver paths.

### Pin and package composition

Seven workspace dependency tags move from contracts-v0.1.41 to contracts-v0.1.43.
All nine contracts lock sources move from `2fcd7739708fe6de1390ff9bd9f90da5a8bc6d87`
to `c69a3190b4d66e84401006e9d73788ff0e837f56`, with no other lock change.
.42 is not pinned separately. The lead handoff records this order after #292 and #297.
Both controls are in the actual base. The public Core contract, frozen files, codec,
probe script, probe binary sources, and test-support API have no .41-to-.43 delta.
RouteClient::progress is the new required harness method; its true result means an accepted byte.
It moves only earlier client bytes, preserves full-stream backpressure, and reads no Core event or route frame.
The testkit also runs workers' ready work. The contracts driver calls every live end without short-circuiting
and preserves the actual Core report for an explicit pump step. R-89 bounds its waiting passes.

Core's job-common.sh exactly equals the pinned file, SHA-256
`0bf5dbc170372d33230a112d0b1562ff17a971e5810f22c5ae46e85cf86acc11`.
Its shared container helper separates online fetches from offline gate work.
Private fetching retains its token boundary; public fetching runs without the token.
Core's fetch-public.sh changes only its explanatory header and retains the libghostty/Zig fetch.
The lead/steward ruling explicitly keeps that fetch file per project.
No production terminal semantics, mutation exclusion, or test branch is added by the pin implementation.

### Source findings and closures

**R299-SI-1 — MEDIUM, closed: discarded bytes reported as delivered.**
At 18334c6f both progress methods compare the length of unsent before and after flush.
Both flush paths can clear pending bytes on terminal error without a successful write.
The real missing-end path also clears them. A discarded buffer can therefore report true,
contrary to R-86. Sol confirms this in source and sends `msg_plugin-w_1791773527_44551f`
and `_7b15ca`; package RC-PIN43-2 matches. No runtime reproduction is run by Sol.
Required behavior is false before any accepted byte and true if an earlier partial write succeeded before an error.

**R299-SI-2 — MEDIUM, closed: progress tests observe private queue state.**
Sol independently reads the initial private unsent length assertions and the real unsent-emptiness loop.
Package RC-PIN43-1 requires public progress and exact peer-visible bytes with a finite bound.
The initial green gate does not close either finding.

Repair `e52255f91363b2f46eee47847270cbc70c7b9456`, then test-only
`3f1fc94d86f6a43c5c5ef310791af676fe5b4ecb`, change only the two route files.
Sol reads the complete repair and every existing flush caller.
Flush now returns the sum of successful Ok(n) writes; discarded bytes add nothing.
The testkit adds both flush counts around run_ready. The real implementation compares its flush count with zero.
Earlier successful bytes remain counted after a later error. Existing callers ignore the new return value and keep their behavior.
Both tests observe public progress and peer bytes, including a failed-stream false result.
The real test checks each byte is x, totals exactly 4 MiB, and has a finite 10000-pass bound.
No assertion or completion condition reads the private queue.
The testkit failure case verifies the peer receives none of the discarded suffix.
R299-SI-1 and R299-SI-2 close in source, matching package correction `89954fffc258e34d405aa00f9eb5bb069f6d5b6e`.
No extra partial-write seam or run is requested; the source preserves the accumulated count explicitly.

Sol reads reversal-progress-e52255f9.log, SHA-256
`4c8c074b9446d801d2e23745d1dd9e3b056cd974cb70c51e705cd9f146185dd5`.
It retains seven failed reversals, including both old length comparisons failing at the discarded-byte assertion,
and ends with zero changed files. It omits full build output, a target-clean history, and each numeric test exit.
These limitations remain; the log supplies no independently proved cache history.

### Pending accounting, fences, and focused removal proof

tm_6_parked_work_does_not_spin leaves testkit pending after its .43 parked_work precondition fence.
It has no PTY or model read. dp_5_full_input_queue stays pending with a lead-approved budget hold,
owned by RC's later large-fill cost repair. Its program-ready and blocked-empty-read conditions remain.
The full-count await_pty for 5242880 bytes precedes the exact input read.
tm_6_begin_and_attach_signal_wake leaves real pending on its separate seeds-0–31 evidence.
Its .43 pty_queued fence precedes the idle assertion; its 26 steps contain no PTY or model read.
Sol reads all three transcripts and verifies zero fence-exception entries for each.

Focused log ...c7a86e70-pool-20261011-193924-35736.log has SHA-256
`b0d6947a9c7a6f577079d9c670a1e58aa80241d003a04686193b81c41f121d03`.
Its actual evidence snapshot is `8dd3f60beb3926d88b792d15ad161784d26f7274`.
Seed-check log ...c7a86e70-pool-20261011-194039-42523.log has SHA-256
`d6642133e30a78bb1087657400c16d5d988871dd59768d4773af14af324cd075`.
Its actual snapshot is `091f80913b1c6a2352d2bbc5409d159d77bb7db4`.
Sol independently verifies each snapshot differs from c7a86e70 only by this pending-line removal,
and each has tree d17c62a1, equal to initial READY head 18334c6f.
The scripts, decoded commands, zero pending-line count, named PASS, and summaries retain seeds 0–31.
The seed check prints both environment values: seed 0 passes in 0.07 seconds and 0–31 in 0.33 seconds.
These are filtered focused proofs, not merge gates or per-command numeric exit proofs.

The historical seed-zero list has 37 identifiers. Sol verifies 36 remain real-pending;
tm_6_begin is the one separately proved removal. Seed zero grants no removal for the other 36.
Sol reports the body overstatement in `msg_plugin-w_1791774035_a794be` and `_5cc0fd`.
Package LOW RC-PIN43-3 matches. The corrected body separates these scopes and the historical A5-4 six PASS changes.
This LOW condition closes after the complete body delta is read.

### Independent A5-4 audit

Sol reads the job and table script without executing either, and independently parses all raw records.
The joined log exactly equals the two named pool logs. The normalized input changes only leading A54/A54T tree tags.
Every table has 691 identifiers, with both cells for every identifier matching the raw results.
Testkit must-pass PASS is 256 to 258, with two changed identifiers.
Real must-pass PASS is 213 to 215; pending-real PASS is 30 to 35, with eleven changed identifiers.
No must-pass PASS becomes non-PASS. These runs use default seeds 0–7 and cover b3cdfffb before and 92d6541d after.
They supply no repaired-head full-corpus execution claim.

One real-pending PASS becomes FAIL: dp_3_data_frames_are_raw, seed 0, step 15.
Expected fffe007b7d; received fffe5e4068c3a97b7d. The result remains FAIL and the id remains pending.
Sol verifies its transcript and step-9 no-program-ready R-60 exception are identical at both tags.
The extra echo diagnosis is source/output inference, not an original event-order trace or a new reproduction.

Before raw log SHA-256: `fac848634fbaf7b669eb11a7ad7060410ea1ddfba12aeeed86b95572bed737ab`.
After raw log SHA-256: `3ef1f96a70e6462b10a15fe1858507ea4f3a14b744b39e131d72a99ffac4ecce`.
Joined log: `79767dbdca7a30bf4dd3c206c50ace6274810e45e18f9cf437dabc352191dfad`.
Normalized input: `d12afae6ca20267a981bbe3a0a248e33f250db594cd79d7116c84da9fc7751d2`.
Real table: `792dcc32111bb4f02783d2c6dc4297485dbfd9cc6142ab3c846603395f2e6db3`.
Testkit table: `d00c33309bc7855cc3baa7be4d9a6f4ce156489506b9285f5869fd37fa409a88`.

### Initial green gate and repaired-head RED

Initial gate ...rc-pin-42-18334c6f-pool-20261011-194125-47145.log:
SHA-256 `e925ff76cd5645411715b8527e69b3c3e87c987713489309fd71349400efda25`.
All ten steps PASS; job and wrapper exit zero after 357 seconds on msa1.
Default 1855 PASS/434 skipped, slow 611 PASS/2043 skipped; baseline 611 PASS/2755 skipped.
Sol independently checks ten unique first outcomes: five caught and five missed.
Stage 2 examines exactly those five misses and catches all five.
All ten unique raw catches equal citations with selected normal and baseline PASS; no FLAKY token.
Held-full reports three budget identifiers at 32 seeds. This gate covers only 18334c6f and not its later repairs.

Repair gate ...rc-pin-42-3f1fc94d-pool-20261011-195433-4196.log:
SHA-256 `31bc5833dc2de45d2fac7daab616462c8682e2f3c508fb394c74d396f46d8186`.
Eight steps PASS, mutants FAIL in the unmutated slow baseline, fuzz NOT RUN.
Job/wrapper exit one after 264/265 seconds on msa1. Default 1855 PASS; normal slow 611 PASS.
Sol independently checks 26 unique first outcomes: thirteen caught, thirteen missed, no timeout or unviable.
All thirteen raw catches match citations and selected normal PASS. Stage 2 tests no mutant.
The baseline has 610 PASS and one FAIL in 42.478 seconds; its interval is 48.7 seconds.
real::resume::slow_tests::the_real_form_compares_both_captures_with_the_raw_output
fails at resume.rs:278, events_pending instead of equal:true. Its earlier normal PASS does not erase this FAIL.
No combined-green mutation result or retry is accepted from this log.

### Separate resume-test source repair and deadline finding

The lead handoff explicitly chooses a separate test-only root fix and an ordinary new-head gate, not a 23al re-gate.
Sol verifies resume.rs, capture_layer.rs, and real/program.rs are unchanged from v1 before this repair.
The failing unit test uses direct Core/harness calls, not RouteClient::progress.
capture_now rejects any polled event except its own capture completion. The late-event mechanism is source inference;
the failing gate contains no event identity trace, and no untouched-v1 failure witness is claimed.

`d8ae1c9d200e2cd7790ce3e1ee61ed39c5dcd91e` adds a test-only quiet wait before the first resume.
Sol reads the complete delta and scratch script/log.
Scratch script SHA-256: `7a9ffc44be44b5227020e2718a69a77574fa157d73c48786130fc0cb65c76164`.
Scratch log SHA-256: `ee0b8fb4545b0455bdb24c8c4dee8629d0c1db5234eb47e4dcbb679ea70942bb`.
The script explicitly adds output on s2 and pumps without polling, ensuring a queued event.
Old test plus that event fails events_pending at the intended assertion, exit 101.
New test plus the same event passes, exit zero; unchanged new test passes, exit zero; final source status is clean.
The script retains PIPESTATUS for the underlying test, but no complete target-clean history.
It proves its synthetic event case, not timing-only 23bl attribution or the original gate's exact event order.
Production R-69 refusal code is unchanged; the later deliberate events_pending refusal test remains.

**R299-SI-3 — MEDIUM, closed: quiet can exceed its cleanup bound.**
At d8ae1c9d the inner pump loop checks no deadline while more stays true.
Sol independently confirms the source defect, matching RC-PIN43-4, and sends
`msg_plugin-w_1791774269_ac2bbf` and `_f1bb39`. No runtime reproduction is run by Sol.
Required change is a deadline check on every continuing pump pass, while preserving event draining and refusal behavior.

Current head `d919844dfa2b74e768987c8cfb980fba0d25a8e8`, tree `49ce20b4c42c3b6febaf8bf0d26374c43e7982a2`,
flattens that loop. Every continuing pass checks the deadline, including more:true.
It polls after each pump, returns only with no more and edges quiet, and waits only with no more.
R299-SI-3 closes. No production resume behavior or exclusion changes.
The d8ae1c9d gate was stopped while queued; it supplies no accepted result.

### Body and package receipts; remaining acceptance

Sol reads the full initial body, raw hash `9ba25949b4c5228bf95699b5736eff128a90c0d3715d5aa16b50371ce05b8443`.
Sol reads the complete current delta, raw hash `a19d5cc946f981c153cfd1528a01d9c82baa13300403abfd333b33e101613e11`.
It records counted delivery, public behavior tests, separate root repair, synthetic-proof limits,
36 retained seed-zero identifiers, the separate wake removal, the baseline RED, and the stopped intermediate job.
The current gate result is explicitly pending. No source or body finding remains open.

Sol reads complete package #299 records at d68916e4, 89954fff, and current
`302ade218da8346f4d6f1d7b21feb2e28e2caf79`, verdicts/rc-real-controls.md.
Its source and LOW closures match. Package receipt `msg_plugin-w_1791774455_093bd7` confirms zero open findings,
but requires the completed current-head gate and matching body result before CLEAN.

One acceptance condition remains: valid completed evidence on d919844d, including its full mutation result and final body.
No running output is read and no earlier gate is substituted for that evidence.
After both initial CLEANs, the final slot still needs one current-v1 import, applicable delta/checks,
one exact-final-head gate, both final reviews, and Q7 before merge.
Sol runs no build, test, gate, mutation, reversal, measurement, or executable base-check job. Never poll.

VERDICT: NOT CLEAN (1 open acceptance condition: completed repaired-head gate; 0 open source or body findings) at d919844dfa2b74e768987c8cfb980fba0d25a8e8 against imported base b3cdfffbba353bf9cfc34f7d7f9b8daccc753035
