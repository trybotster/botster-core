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

## Round 2 — surviving progress mutation and test-only repair

Completion/repair receipt: `msg_plugin-w_1791775399_c56186`.
Completed d919844d gate: `gates/botster-core-stage1-rc-pin-42-d919844d-pool-20261011-200604-66397.log` under `~/botster-sessions`.
SHA-256: `25c4aa1c7be388f5a0bbb2cc68719428c9cfdf4b194d36e6a3ee7cc74a1ac9ba`.
The header names head d919844dfa2b74e768987c8cfb980fba0d25a8e8 and gate reference bfef88653bb59ad8d3cd38da1e206f7aba64ce92.
The imported ancestor remains b3cdfffb. The job runs on MS-A1, kernel 6.12.111+deb13-amd64.
Eight steps PASS, mutants FAIL, and fuzz NOT RUN. Step total is 602.7 seconds.
Job/wrapper exit one after 833/834 seconds, with 216 seconds queued and 617 seconds running.
Default has 1855 passed and 434 skipped in 17.856 seconds. Slow has 611 passed and 2043 skipped in 44.772 seconds.
The corrected resume test passes in the unmutated baseline: 611 passed, ten slow cases, 2755 skipped, in 43.226 seconds.

Sol independently parses all 26 unique first outcomes: thirteen caught and thirteen missed, with no timeout or unviable.
Stage 2 examines exactly those thirteen misses, catches twelve, and misses one, in 320.4 seconds.
All 25 raw catches match the 25 named citations and normal PASS records; all twelve slow catchers also have baseline PASS.
No FLAKY token occurs. The surviving mutant replaces + with - in TestkitRoute::progress at route_client.rs:359.
The gate remains RED. A passing baseline does not close a surviving mutation.

New head: `7c7dadd165ae005f5e6e19cd99d752b5bb3b9d7a`.
Sol reads the complete two-file test-only delta and reversal record.
The new test gives the route eight bytes and reads ab from its peer before progress.
The first flush moves ef; a test reader receives cdef during the real run_ready path; the second flush moves gh.
The test observes progress and peer-read bytes, then false progress while the peer receives gh.
It checks no private queue or worker-machine state. Workers::add_node is cfg(test), and the existing simulation step bound remains.
Production progress, pending lists, pins, CI, and exclusions do not change.
No new source finding is added. The completed new-head mutation result must close the remaining acceptance condition.

Reversal SHA-256: `b188b4212859e8a2dfc452d673bdd3e1fa1df630aa216cf1763e4a4dbe45a11b`.
The log names the exact head and targets/rc, retains four failed reversals, and ends with restored source status.
The subtraction, >= comparison, always-false progress, and empty injection method fail this new behavior test.
Multiplication passes this new test. Its earlier gate catch remains separate; the log claims no new multiplication catch.
The filtered record supplies no per-command numeric exits, full build output, or complete target-clean history.
Sol reads complete package repair receipt `2a79e846903e897dbb52f0eebf7adc3a2a001fcc`, including the d919 RED and limits.
The source disposition matches. RC-PIN43-1 through -4 remain closed.
Completed exact-head gate and matching final body remain due; no running 7c7dadd1 output is read.
The final-slot import after merged #291 remains due. Sol runs no execution job. Never poll.

VERDICT: NOT CLEAN (1 open acceptance condition: completed new-head mutation/gate closure and final body; 0 new source findings) at 7c7dadd165ae005f5e6e19cd99d752b5bb3b9d7a against imported base b3cdfffbba353bf9cfc34f7d7f9b8daccc753035

## Round 3 — completed repaired-head gate and body

Head: `7c7dadd165ae005f5e6e19cd99d752b5bb3b9d7a`.
Tree: `1646e3c80c5843854069d8ae622d3324de847628`.
Imported base: `b3cdfffbba353bf9cfc34f7d7f9b8daccc753035`.
Gate comparison reference: `bfef88653bb59ad8d3cd38da1e206f7aba64ce92`; it is not an imported ancestor.
Completion receipt: `msg_plugin-w_1791775998_ad7bdc`.
Rounds 1 and 2 cover the complete source and all repairs at this unchanged head.
Sol reads completed gate headers, list/coverage results, every mutation outcome and citation,
relevant normal PASS records, baseline records, conformance summaries, step summary, and final exits.

Gate: `gates/botster-core-stage1-rc-pin-42-7c7dadd1-pool-20261011-202300-83199.log` under `~/botster-sessions`.
SHA-256: `c31d96e03e2fd1d1aa4630ae8f905d2d9e7ea2456ec5baee0246b532b8f3b8c8`.
The header names this exact head and gate reference. The lists step independently names the actual b3cdfffb merge base.
The job runs on MS-A1, kernel 6.12.111+deb13-amd64, with both parallelism caps set to four.
All ten steps PASS in 572.6 seconds. Job and wrapper exit zero after 585 seconds, with one second queued and 584 running.
Default: 1856 passed, 434 skipped, 18.756 seconds. Slow: 611 passed, 2044 skipped, 45.020 seconds.
The unmutated slow baseline has 611 passed, ten slow cases, and 2756 skipped, in 44.238 seconds.
The corrected resume test and both route progress tests have normal PASS; real tests also have baseline PASS.
Coverage checks 1077 slow-only tests and four named real proofs.

Sol independently parses 26 unique first outcomes: fourteen caught and twelve missed, in 74.8 seconds.
Stage 2 examines exactly those twelve misses and catches all twelve in 284.6 seconds.
All 26 unique raw catches equal all 26 named citations. Each catcher has normal PASS.
Every slow catcher also has unmutated baseline PASS. No unmatched identity, remaining miss, timeout, unviable, or FLAKY token occurs.
The subtraction mutant that survived d919 is caught by progress_counts_the_bytes_that_move_after_the_workers_read.
Multiplication is caught by the earlier progress test, as the body states.
Workers::add_node is cfg(test); no gate mutation is claimed for that helper.
Fuzz PASS reports no changed decoder harness, not a new fuzz execution.

Both ordinary conformance summaries have 257 passed and zero failures.
Held-full runs three budget identifiers at 32 seeds: 96 expected ignored results, zero failures, in 22.79 seconds.
The dp5 budget hold remains scoped to the later cost repair; it does not become a pending removal.
Sol independently verifies all 35 reported real-pending passes stay in the 43-entry list.
The reviewed focused wake-removal proof remains separate. Minimum results are testkit-passing 65/69,
real-passing 59/68, and real-accepted 60/69. These counts cover the actual imported base, not later v1 imports.

Sol reads the complete final body delta against the already reviewed d919 body.
GitHub reports this exact head and base reference bfef8865. Live body SHA-256:
`3fb84caff8cb2f7b7e99230dc7ce4d53ab12535c3e88b31b480fbd5fe1ae1299`.
The body adds the two-send proof, the d919 RED, its surviving mutation, and the completed 7c gate.
It preserves the prior baseline RED, stopped intermediate job, synthetic-event scope, multiplication reversal PASS,
partial-write test limit, historical A5-4 scope, and separate focused removal evidence.
It identifies b3cdfffb as the last imported v1 and bfef8865 as the gate-time reference.
The final-slot import remains explicit.

Package LOW RC-PIN43-5 corrects the historical 3f gate classification.
The earlier body called all thirteen stage-1 misses real-tier mutations. One is the testkit subtraction mutation.
Sol reads all thirteen original MISSED records and verifies twelve real-tier identities plus that one testkit identity.
Owner correction receipt: `msg_plugin-w_1791776096_46fcf9`.
Sol reads the complete published correction. Current live body SHA-256:
`999b41f72f42ad2890873b6c6aeab7bd27dcda97f5094bfe11b47452cbb5d3df`.
It gives the correct classification and preserves the baseline FAIL and absent stage-2 mutation result.
This LOW closes without source change or execution. No source, body, or completed-evidence condition remains open.

Sol reads the complete current package verdict and intervening integration receipt at
`f7e9923c6d11a89f26d500180fc3a512009b54ec`, verdicts/rc-real-controls.md.
Package completion receipt: `msg_plugin-w_1791776184_432338`.
Its exact-head CLEAN, all source/LOW closures, completed gate, and retained scope limits match this review.
The two reviewed removal identifiers have normal PASS records in their applicable tiers.
Both current source reviews now have zero open findings. The completed gate and final body close the prior acceptance condition.
The final-slot current-v1 import, applicable delta/checks, exact-final-head gate, both final reviews, and Q7 remain due before merge.
Earlier scope limits stay unchanged. Sol runs no execution job. Never poll.

VERDICT: CLEAN (0 open integration findings; current source, completed evidence, and body) at 7c7dadd165ae005f5e6e19cd99d752b5bb3b9d7a against imported base b3cdfffbba353bf9cfc34f7d7f9b8daccc753035

## Round 4 — final-slot base import and composition

Head: `f16a10ba028ca3ea6b0139d58aabea7cb5e051d5`.
Base: `fcc09461cb0c0de4ef067c160f24d205f0f65c30`.
Tree: `67c34fba86d161521be0360ae5829e70807631a4`.
Reviewed parent: `7c7dadd165ae005f5e6e19cd99d752b5bb3b9d7a`.
Previous merge base: `b3cdfffbba353bf9cfc34f7d7f9b8daccc753035`.
Receipts: `msg_plugin-w_1791777459_d4e33c` and `msg_plugin-w_1791777540_819e7d`.
The lead assigns this slot after #293. The merge has these exact reviewed/base parents and no subsequent source edit.
Sol independently verifies both merge bases and reads the complete overlapping lock/list delta.

### Independent composition audit

The base changed 55 paths; the PR's own set remains eleven paths.
Their only overlaps are Cargo.lock and core-pending.txt. Every outside-own blob equals fcc09461.
Nine own blobs equal the reviewed head. Only the lockfile and pending-file blobs change through the import.
The complete own patches match after removing blob-index lines and hunk headers.
Both diffs have 281 additions and forty deletions across eleven files.
Old full-index own-diff SHA-256: `09c99e6220d4f414f9b144dea3bd2a01da6ff9efb5853da396e03b512640d86a`.
New full-index own-diff SHA-256: `ed3ad596a0299e63fc0a5caadf626f9b66b15e0bfb2bed649e9d7154f6d8d571`.
The lock retains all imported botster-test-process dependencies and all nine .43 contracts source lines.
Seven workspace tags remain .43. The eighteen own lock change lines remain identical.
The route client, real route, real resume, and worker blobs retain their exact reviewed versions.
No source repair, test change, private assertion, or new control enters the own delta.

Sol independently applies the three-way set formula to all nine conformance files.
The PR's parked-work removal and the base's two e3_1 removals remain removed, with no common removal.
Pending counts compose as 412 at the old base, 411 at the reviewed head, 410 at the slot base, and 409 at the merged head.
The real wake removal remains: 44 at both bases, 43 at both PR heads.
The budget-held addition remains. Ledger 691 and minimum 69 remain unchanged.
No removed identifier returns, and no unreviewed pending removal appears.

The #291 import retains accepted merge e4f49504 and its ordering resolution.
The #293 import equals accepted merge fcc09461. Sol independently verifies its exact e4f49504/029a740b parents and gated tree f2c6afb1.
Sol reads Sol-B's complete final evidence and merge records b6b92593/f595cd23, and receipt `msg_plugin-w_1791777684_5164d0`.
The build-root implementation remains in botster-test-process; testkit re-exports it.
The conformance suite checks its binary and nine linked libraries before argument parsing and trial dispatch.
Sol reads that imported interface and verifies equality with the accepted base.
The .41-to-.43 test-support API had no delta in Round 1; this merge adds no workspace library to LINKED.
Candidate builds retain their canonical-root directories and source-root check.
The separate coverage guarantee, historical FLAKY misses, PC-owned fence work, and earlier Mac/bootstrap/disk scopes remain unchanged.
This review does not accept a new coverage guarantee or broaden those evidence scopes.

The imported mutation file retains the accepted narrow build_worker whole-body replacement exclusion from #293.
Sol reads its complete delta. It covers process glue, not progress, the resume wait, or their behavior tests.
Earlier in_8 FLAKY records remain missed and owned; this import does not turn them into catches.

### Canonical check and local supplementary evidence

Sol reads the complete canonical log, SHA-256
`581086b03e571d07bddbea3b4eabe3579c7fbd043c6202c01ac9a94a2887ef07`.
Both ancestry checks and condition 1 PASS. Condition 2 FAILS on Cargo.lock overlap.
Condition 3 FAILS at the first raw difference, the lockfile's index line. Both condition-4 list rules PASS.
The command exits one and result stays FAIL. Both explicit delta reviews are required; no automatic carry is used.

Sol reads local-final.sh, SHA-256 `7af71c913d468de06c981a757f7b0335e4a45e1f9593e034859f768d241f0d51`.
It exports both caps at four, unsets RUSTUP_TOOLCHAIN, and calls cargo clean before the logged checks.
The clean has no printed numeric exit or retained raw output. Its time marker alone is not an independent clean-exit proof.
The body retains that limit. No target-writer history beyond this record is inferred.
Local log SHA-256: `7e8cf33965e28a8562e3e7de8174c40d8c517abcdff50bc626fbe40944948e13`.
It names this exact head/base and reports ten command exits, all zero.
Static results include fmt, lists, ledger-ids, taint, and clippy. Prebuild writes three candidate binaries.
Library suites report host 332, testkit 426, worker-core 193, and Core 14 passed.
Real route/resume tests report 26 passed. Strict seeds 0–31 report 259 passed and zero failures in 6.60 seconds.
Strict checks 321 of 339 pending identifiers, excluding eighteen real-only cases, with four held.
These local results are supplementary; they do not replace the exact-final-head gate.

### R299-SI-4 — LOW, closed: raw diff metadata omitted from the body

The initial slot body says that outside Cargo.lock only one pending hunk header differs.
Sol independently finds two changed index lines, five changed lock hunk headers, and one changed pending hunk header.
The canonical log prints only the first difference. Sol requests correction in `msg_plugin-w_1791777752_04bc56` and `_07ca1a`.
Owner correction receipt: `msg_plugin-w_1791777771_bee79c`.
Sol reads the complete published body delta. It names all metadata classes and preserves identical content/change lines.
It also preserves the clean's unprinted-exit limit. R299-SI-4 closes without source change or new execution.
Initial slot body SHA-256: `9752e138ea2b9ca549b9b1db0c5c615002b830af1a79f0fadbd65329161f6565`.
Corrected body SHA-256: `ed06f2597a305a5f1b6809d030560bf186fa6a977c87e571b4ee9c6e4e1d6caa`.
GitHub reports this exact head and imported base.

Sol reads complete package final-slot delta at `92d9a2c394d28adcb3f85418f8c1710da9f9a152`.
Package receipt: `msg_plugin-w_1791777840_e90fab`.
Its source, all nine sets, lock composition, narrow exclusion, body correction, and retained limits match this review.
Both delta reviews have zero open findings. The canonical check remains FAIL with these explicit delta reviews.
The ONE completed exact-final-head gate, full mutation result, matching final body, both final verdicts, and Q7 remain due.
This round accepts no final-slot gate result, later head, or different tree. Sol runs no execution job. Never poll.

VERDICT: CLEAN (0 open integration delta findings; final evidence remains due) at f16a10ba028ca3ea6b0139d58aabea7cb5e051d5 against base fcc09461cb0c0de4ef067c160f24d205f0f65c30

## Round 5 — corrected profile delta after the failed final gate

Head: `77eae642eaac0c56cde035870616d17bf2f877ba`.
Parent: `edb9de1074c2df0086f212ea2cbd9695667894ce`.
Tree: `9423da03abc7b905308920570cf767216e0f3ee5`.
Imported base: `fcc09461cb0c0de4ef067c160f24d205f0f65c30`.
Prior source-delta head: `f16a10ba028ca3ea6b0139d58aabea7cb5e051d5`.
Owner requests: `msg_plugin-w_1791779773_85a0a7` and correction `msg_plugin-w_1791779864_52aa29`.
Complete package profile review: `e5a6ef563afc877ee95589287553bf2530901c11`.
Complete package corrected delta: `cd036b066e4ce6c2f2e9493bc2ddb5fa5d242afc`.
Package corrected receipt: `msg_plugin-w_1791779909_a4bb7a`.

### Failed final gate — retained RED

I read the completed f16a10ba gate header, default test outcome, budget failures, CI summary, and final job records.
Log: `~/botster-sessions/gates/botster-core-stage1-rc-pin-42-f16a10ba-pool-20261011-210606-1524.log`.
SHA256: `94745be6bae0e597019333d3bb714406716817f84c45d905e9d456b1bd252186`.
The exact head and base match the Round 4 source review.
The first six CI steps PASS. Test-budget FAILS; slow, mutants, and fuzz do not run.
Nextest runs 1914 tests: 1913 PASS, one timeout, 432 skipped, 25.614 seconds.
`conf::in_9_legacy_keys_table` times out at 2.005 seconds against the two-second limit.
The gate reports two budget failures: nextest's failure and the duration failure for that same test.
CI total: 386.9 seconds. Job exits 1 after 659 seconds on node linux, with 156 queued seconds and 503 run seconds.
The wrapper exits 1 after 660 seconds.
This gate supplies no final acceptance and no full mutation result.

### Profile delta

I read the complete eighteen-line Cargo.toml addition and the one-line comment correction.
The full-index diff from f16a10ba to this head has SHA256 `7b51c82f001f5dc8e718f0c7d68951312c52afb254f2482d6f3c4e293062a364`.
Cargo.toml is the only changed path. Every other source blob retains the Round 4 version.
The lead's handoff approves this HIGH mitigation: optimize the two contracts conformance dependencies at level 2.
The steward accepts the mitigation and assigns the per-step clone cost to contracts backlog B-1.
RC retains separate Core work for `longest_key_sequence`. Those follow-ups are not accepted by this delta.

The two new package overrides are `botster-core-conformance` and `botster-conformance`.
Both set `opt-level = 2`, `debug-assertions = true`, and `overflow-checks = true`.
No Core workspace package receives an override. Existing Core package compilation settings remain.
The target-head manifests put these dependencies in testkit, xtask, and Core's dev dependencies.
No product state machine, assertion, gate budget, control, status list, mutation exclusion, or coverage rule changes.
The configuration follows the existing dependency-override pattern and the explicit lead ruling.

I read the sample header and relevant clone, driver, event, input-log, matcher, and encoder stacks.
Sample SHA256: `bb9338d8a2cce72809ae9f233f80a9d0c1761c5d829eb15e4179045aae76eb89`.
Inclusive summary SHA256: `dc9f70cd0673150a3445a27ae2ce9a97e56d1c5ed15792178f8bb1839da45720`.
The sample has 5919 test-thread samples. The stacks support repeated driver work and separate encoder cost.
Inclusive stack counts overlap; I do not sum them as independent percentages.
The owner attributes this sample to 1024 seeds, distinct from the timing comparisons at seeds 0–31.
The owner reports Mac timing changes of 1.72 to 1.00 seconds for the id and 28.40 to 21.48 seconds for one-thread strict conformance.
These reports do not establish a completed Linux gate or final budget acceptance.

### Precision findings — CLOSED

Package RC-PIN43-6 LOW identifies a sample-seed inconsistency in edb9de10: source comment 32, body 1024.
I read the complete 77eae642 correction. It changes only that comment to the owner's 1024-seed sample description.
The body keeps the separate timing comparisons at seeds 0–31. The profile values do not change.

Package RC-PIN43-7 LOW identifies the body statement that both failed-gate durations are 660 seconds.
I independently read the raw job 659-second and wrapper 660-second records.
The corrected body reports those durations separately. It retains RED and the absent later steps.
The body also names the comment-only head and retains PENDING for its final gate.
Both LOW findings close without additional execution.

I read the complete edb9de10 body, then every changed line of the corrected published body.
Corrected raw JSON SHA256: `0abc7eaa66e1d7d9534c43e78b860468a89812d14e4c05aabf31b4320dbe32f0`.
GitHub names this exact corrected head and imported base.
The body keeps all earlier source, canonical-check FAIL, gate, cache, and evidence limits.

### Local supplement

I read the complete local-check script and statically audit the log's header, commands, exits, and summaries.
Log: `evidence/rc-pin-42/local-final-edb9de10-20261012-042920.log`.
SHA256: `e9084ecc30a606331c729b1be7fa4755cbd748f218d57e1a7c98934acf546d79`.
The log names edb9de10 and base fcc09461. The later correction changes only a comment.
All ten printed command exits are zero.
Library results: 332 host, 426 testkit, 193 worker-core, and fourteen Core passes. Route/resume slow tests: 26 passes.
Strict conformance at seeds 0–31: 259 PASS, zero failures, 3.18 seconds, exit zero.
The strict check runs 321 of 339 pending ids; eighteen are real-only and four are held.
The script sets both parallelism caps and removes the toolchain override.
It prints the clean time but no numeric clean exit or independent raw clean output. That limit remains.
This supplement does not replace the final gate or establish every target writer.

### Disposition

No integration source-delta or body finding remains, including LOW findings.
The complete corrected package verdict agrees.
The Round 4 import, lock composition, status-set checks, and canonical FAIL remain applicable to every unchanged path.
This HIGH profile delta is CLEAN on the exact corrected head.
The lead keeps this PR in its slot and requires both delta CLEANs before ONE new final gate.
The completed exact-head gate, full mutation result, matching final body, both final evidence verdicts, and Q7 remain due.
The earlier f16a10ba RED cannot carry acceptance to this head. No later head or final gate is accepted here.
I ran no build, test, gate, mutation, reversal, measurement, or executable base check.

VERDICT: CLEAN (0 open source-delta findings; final acceptance remains due) at 77eae642eaac0c56cde035870616d17bf2f877ba against base fcc09461cb0c0de4ef067c160f24d205f0f65c30

## Round 6 — completed final gate and final evidence acceptance

Head remains `77eae642eaac0c56cde035870616d17bf2f877ba`.
Imported base remains `fcc09461cb0c0de4ef067c160f24d205f0f65c30`.
Reviewed source tree remains `9423da03abc7b905308920570cf767216e0f3ee5`.
Owner completion receipt: `msg_plugin-w_1791780845_f66e4d`.
Round 5 and package `cd036b066e4ce6c2f2e9493bc2ddb5fa5d242afc` cover the exact source delta.
Complete package matching receipt `840e018668db17bf7c59c7951d6c37e880c2f8fc` is also read.

I read the completed final gate header, outcomes, summaries, mutation records/citations, and final job records.
Log: `~/botster-sessions/gates/botster-core-stage1-rc-pin-42-77eae642-pool-20261011-214255-19819.log`.
SHA256: `5d27138eebfb9ae3f40ee105eee61061b883103e840393ab5d986366f96b26e8`.
All ten steps PASS in 610.0 seconds.
Job and wrapper exit zero after 627 seconds on msa1. The job has one queued second and 626 run seconds.
Default: 1914 PASS, 432 skipped, 9.748 seconds. Slow: 616 PASS, 2060 skipped, 39.014 seconds.
`conf::in_9_legacy_keys_table` has normal PASS in 0.962 seconds, within its unchanged two-second limit.
Both strict summaries have 259 PASS and zero failures.
Prebuild writes the three-binary candidate manifest.

I independently match all 26 unique first mutation identities: fourteen caught and twelve missed.
Stage 2 tests exactly those twelve misses, all in real/route.rs, and catches all twelve.
All 26 raw catches equal their citations. Every cited test has normal PASS.
All twelve stage-2 catchers also have baseline PASS.
Baseline: 616 PASS, eleven slow, 2810 skipped, 40.977 seconds.
The mutation stage durations are 80.9 seconds, 62.9 seconds for baseline, and 252.1 seconds for stage 2.
No mutation remains missed, timed out, or unviable. The log has no FLAKY token.
Fuzz selects no changed crate with a decoder harness.

Both original removal ids have normal testkit and real PASS: tm_6_parked_work and tm_6_begin_and_attach.
All 34 reported passing ignored real ids remain in the unchanged 43-entry real-pending list.
Held-full runs three budget ids at 32 seeds: 96 expected ignored results, zero failures, 21.74 seconds.
The full-input queue id remains pending and budget-held. No new removal follows from the profile mitigation.

I read every changed line of the final body after the complete Round 5 body review.
Final raw JSON SHA256: `23d1eeac493ad72fe9c9064142060d6be86ea784e7ef39cd5d507fac28a47934`.
GitHub names this exact head and base.
Gate entry 9 now reports the exact-head completed PASS, mutation identities, legacy-key duration, held-full scope, and final job durations.
The final-slot section names this same completed gate. Earlier RED gates and all source/cache/evidence limits remain.

No integration source, body, or evidence finding remains.
I read complete package final acceptance at `4a9011bf0b2485f7eb1e6b86ecd17b7a877d5ea7`.
Package receipt: `msg_plugin-w_1791780944_b08a7a`.
Its exact source identity, completed final gate, mutation audit, body comparison, and retained limits agree with this review.
Both final reviews have zero open findings. The prior default-budget acceptance condition closes on this completed exact-head gate.
The canonical base-merge-check FAIL retains both explicit source delta reviews.
The earlier local clean-proof, reversal, synthetic-event, attribution, and platform limits remain.
Q7 and an equal-tree merge receipt remain due under the lead's role.
This record accepts no later head, source tree, gate, or merge.
I ran no build, test, gate, mutation, reversal, measurement, or executable base check.

VERDICT: CLEAN (0 open) at 77eae642eaac0c56cde035870616d17bf2f877ba against base fcc09461cb0c0de4ef067c160f24d205f0f65c30
