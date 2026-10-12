# Integration review — #302, route semantic input

## Round 1 — source boundaries, conformance, and first gate

PR: https://github.com/trybotster/botster-core/pull/302. HIGH: worker-core behavior.
Head: `c4ea4032456ed26605da3efb6fbabf1ba1061b6d`.
Parent: `8c7cfa75180689ad568823078ee4e598eadb7a19`.
Tree: `563e5850ec0690a1386c6cacb47e57befce66e72`.
Actual source merge base: `bfef88653bb59ad8d3cd38da1e206f7aba64ce92`.
GitHub/gate base reference: `e4f495040a80b524a7ac9accba95272d5c2594fa`; it is not an imported ancestor.
READY: `msg_plugin-w_1791777045_d571b5`.
The package reviewer is 0165, per the lead's correction relayed in `msg_plugin-w_1791777192_efcb77`.
017e withdraws from this PR and retains #299. Sol remains the integration reviewer.

Sol reads the complete five-file diff: 207 additions and thirteen deletions.
Full-index SHA-256: `5acb9b24f5a296799b169a1456f1aa379c16c84fbd804de4564574f2515e16a6`.
Sol reads affected route decoding, receipt, admission, encoding, retention, refusal reservation, and allowance paths.
Sol reads the codec's key validation, UnsupportedWhat serialization, CoreLimits delivery, host key admission,
pinned DP-5/IN-9, amendment-15 A15-1, R-53, R-75, and the lead's PR A/PR B scope.
No reviewer build, test, gate, mutation, reversal, measurement, or executable base check runs.

### Interfaces and intended behavior

Route::decide now maps Key, Mouse, and Focus into the existing InputPayload forms.
It preserves the frame's optional op and every applicable semantic field. A route key has no host-only repeat.
These inputs use the same ordered admission point as host input and model replies.
The worker encodes at the start of the transaction through the existing libghostty model path.
No second terminal encoder, handwritten expected terminal sequence, test branch in production, or new mode tracker appears.
start_route omits NotReported with no write or frame. Unsupported preserves the encoder's serialized what and the route op.
ModePreconditionFailed remains unchanged. View-only, before-live, stopping, and ended-session checks remain before encoding.
Resize remains outside PR A, as the lead approved. PR B and the separate DP-12 focus-state gap remain due.

Four new behavior tests cover a written key, omitted mouse/focus, an encoded mouse, and an unsupported coordinate.
Expected terminal bytes come from the existing model encoder. The unapplied-kind test now uses resize.
These tests do not cover the two size/retention defects below. The body reports old-source reversals,
but the named evidence directory contains no retained four-test reversal record. Sol claims no independent runtime reversal proof.

### R302-SI-1 — HIGH, open: queued semantic input bypasses the input-byte bound

This finding matches package F142. Sol independently identifies the accounting gap before the package receipt.
At input.rs:72–77, payload_len counts only Bytes, Text, and Paste. It returns zero for Key, Mouse, and Focus.
The new Route::decide branches place these semantic inputs in Pending::Route.
At input.rs:141–155, route_bytes uses that zero for every queued semantic input.
At route.rs:789–802, read_allowances subtracts route_bytes and the reader's pending bytes from the effective input bound.
Once a complete key frame is decoded and queued behind a blocked host write, its text remains retained,
but the reader no longer holds its frame and route_bytes charges zero. The input allowance returns.
Repeated queued key frames can therefore exceed DP-5/R-53's input-byte bound while Core continues to read.
The separate refusal reservation counts queued transactions. It does not enforce their retained input bytes.
The codec limits a key's scalar fields and modifier set, but its Key.text remains an allocated String.
This is a source finding. Sol runs no runtime reproduction.

Required change: account for retained semantic input when it waits at admission.
Preserve encoding with the modes at transaction start; do not encode early to compute the charge.
Add a behavior regression that blocks a preceding write, fills semantic input to the effective bound,
observes input backpressure, then releases the write and observes resumed input with exact order and no loss.
The byte bound and the refusal reservation must both remain effective.

### R302-SI-2 — HIGH, open: the route bypasses the configured key-text bound

This finding matches package F144. Sol independently checks its source and contract chain.
Pinned amendment-15 A15-1 limits key text to CoreLimits.max_key_text_bytes of UTF-8.
A route key over the bound must receive input_refused{too_large}, with no effect and the route open.
The bound applies before other key size work. A key at the configured bound is admitted.
Worker::on_launch receives CoreLimits through LaunchSpec.limits, so the worker has the configured value.
The host's begin path checks this bound in admit.rs, but route input bypasses host begin.
The new decide -> enqueue -> start_route -> model::encode path has no max_key_text_bytes check.
Sol verifies that worker-core has no reference to max_key_text_bytes.
The codec's Input::check validates key scalars and modifiers, but it checks no text length.
The frame bound is a different bound and can permit text above the configured key limit.
The new route path can therefore reach encoding without the required too_large refusal.
This is separate from R302-SI-1's retained queue accounting. Sol runs no runtime reproduction.

Required change: enforce the configured key-text byte bound on route input before other key size work.
Preserve the optional op, send too_large with zero effect, and keep the route open.
Add configured-boundary and one-byte-over behavior tests. The over-bound case must write nothing.

### Conformance accounting and retained local evidence

Sol independently compares identifier sets against the actual bfef8865 source base.
Three identifiers leave pending on both-tier proof: dp_5_not_reported_sends_no_frame,
dp_5_silent_not_reported_is_omitted_not_written, and in_1_host_write_and_route_frames_share_payload_kinds.
dp_4_worker_encodes_route_frames_with_write_time_modes leaves testkit pending and enters real-pending.
No other identifier enters pending or leaves real-pending. Counts are 406 pending and 45 real-pending.
The dp_3 route-codec identifier stays pending for the separate resize path.
The first source commit is 28c7be7737c5ab36c3e93f2424ab5c79d2f95e10 on bfef8865.
Its three worker source/test blobs equal this reviewed head; the two later commits change lists only.

Sol reads all four local scripts, completed logs, relevant raw failure context, and three removal transcripts.
Sol independently matches all 46 after cells and all eight before cells to their raw summaries and numeric command exits.
The after run covers 23 identifiers. The before run covers only four identifiers.
Each run records a target clean with exit zero and its own source and worktree.
The script patches seven contracts crates to the clean .41 tree at 2fcd7739708fe6de1390ff9bd9f90da5a8bc6d87,
sets both parallelism caps to four, sets seeds 0–31, unpends the selected identifiers, and restores tracked files.
Sol matches all 23 printed transcript hashes to that tree. No modified transcript is accepted.

All three removal identifiers have before FAIL on both tiers and after PASS on both tiers.
dp4 has testkit before FAIL/after PASS and real before FAIL/after INCONCLUSIVE at step 20.
The raw after totals are seven PASS, twenty-one FAIL, fifteen INCONCLUSIVE, and three unsupported-control results.
Each of the fourteen a3_2/dp_12 cases has testkit FAIL and real INCONCLUSIVE.
The remaining dp4 real result supplies the fifteenth INCONCLUSIVE.
dp3 has after FAIL at step 23 on both tiers; its before cells were not run.
The dp4 transcript has the recorded R-56 step-24, R-57 step-9, and R-60 step-12 exceptions.
Its blocked two-byte host write also lacks the R-74 hold. The body assigns its fence repair to P3 with PC.
These records establish the current pending disposition; they do not supply an original event-order trace or a new fence repair.
DP-12, pty_fail_after, control_queue, success-acknowledgement, and optional-op failures stay outside this source repair.

After log SHA-256: `dd79e0144ea6deff4d71399faaa656a96a8f836470bacef4b4bee54aa83321ce`.
Before log SHA-256: `45c1f095b6c5d5652ab791b4f8819717008f68d959347df82279eb82997075e2`.
Local check SHA-256: `7cbf2f13ad20d17f364c2b432cfc7a0952e9e383279e2d490ecb98d6798bc24b`.
The local check names 8c7cfa75, records a clean exit zero, 1202 nextest passes and exit zero,
then strict 261 PASS/one FAIL and exit 101 because dp4 is pending but passes.
The c4 list-only commit moves dp4. Its unretained terminal rerun remains an owner report, not this log's result.
The exact-head completed gate supplies the later strict proof.

### Completed first gate

Gate: `gates/botster-core-stage1-p3-route-semantic-input-c4ea4032-pool-20261011-204236-1219.log` under `~/botster-sessions`.
SHA-256: `3eb7c10edc4e03c4cbaf3eb2b9880ac5baba0fd23f28ca23932540a2a4c35fca`.
Sol reads exact headers, list/coverage results, all mutations and citations, relevant normal tests,
conformance summaries, step summary, and final exits. The lists step names the actual bfef8865 merge base.
All ten steps PASS in 313.7 seconds on MS-A1, kernel 6.12.111+deb13-amd64.
Job and wrapper exit zero after 412 seconds, with one second queued and 411 running.
Default: 1869 PASS, 429 skipped, 14.945 seconds. Slow: 613 PASS, 2039 skipped, 42.298 seconds.
Both strict summaries have 262 passed and zero failures. Held-full has 64 expected ignored results,
zero failures, two budget identifiers at 32 seeds, in 19.27 seconds.
Coverage checks 1076 slow-only tests and four named real proofs.

Sol independently parses five unique mutation identities: four caught and one unviable, in 5.3 seconds.
All four raw catches equal their citations and have normal PASS. No miss, timeout, or FLAKY token occurs.
Stage 2 does not run because stage 1 has no miss. Fuzz reports no changed decoder harness.
All four new worker tests PASS. All three both-tier removal identifiers PASS in normal testkit and real trials.
dp4 has normal testkit PASS. All 34 observed real-pending passes remain in the 45-entry list.
This green gate does not exercise or close either HIGH source finding and does not carry to a repair head.

### R302-SI-3 — LOW, partially closed: body source and outcome precision

Initial live body SHA-256: `d4db44f93ea6160d87c9bb18ac6f6ca24c8072f585d3b77efdf3328264d762eb`.
Sol reports the baseline/local/base conditions in `msg_plugin-w_1791777180_de1c9d` and relays them to 0165.
Owner correction receipt: `msg_plugin-w_1791777212_5ff211`.
Sol reads the complete published delta, SHA-256 `8e2fc72a5eaefaccfb5d52de45ac9fc6bdd626882a4d1f034c56cfac4417d272`.
The corrected body names 0165, limits before coverage to four identifiers, and marks dp3 before cells not run.
It preserves the 8c strict FAIL and marks the c4 rerun as an unretained terminal result.
It distinguishes the actual parent 8c, source merge base bfef, and GitHub/gate reference e4f49504.
Those subconditions close without source change or new execution.

One LOW condition remains: the body describes all fourteen focus cases as the same FAIL/no-matching-event result.
Their real-tier raw results are INCONCLUSIVE under step_timeout; their testkit raw results are FAIL.
Sol independently verifies that tier split and requests it in `msg_plugin-w_1791777256_197c75` and `_97c492`.
Required change: preserve the exact outcome on each tier. No new execution is requested.
Package F143 covers this same body condition and the corrected subconditions above.

The two HIGH source findings and the remaining LOW body condition block CLEAN.
Sol reads complete package Round 188 at `1528ccab0b63581d17d8080b9d9989ac5f356a76`, verdicts/p3-worker.md.
Package receipt: `msg_plugin-w_1791777531_413f66`.
Its three open findings, matching F142/F144/F143, and evidence limitations agree with this review.
After repair, the applicable source/mutation evidence, completed exact-head gate, and both reviews remain required.
The final-slot current-v1 import/checks, exact-final-head gate, final reviews, and Q7 remain due before merge.
Sol runs no execution job. Never poll.

VERDICT: NOT CLEAN (3 open: 2 HIGH source findings and 1 LOW body condition) at c4ea4032456ed26605da3efb6fbabf1ba1061b6d against source base bfef88653bb59ad8d3cd38da1e206f7aba64ce92

## Round 2 — repaired source and corrected body

Head: `78d7057fca48480e2e8e3174baa2e5c2ab7a70c3`.
Parent: `c4ea4032456ed26605da3efb6fbabf1ba1061b6d`, the previously reviewed head.
Tree: `7760d25cadb6cd76f8cc6b01d0269fab15f03e79`.
Source base: `bfef88653bb59ad8d3cd38da1e206f7aba64ce92`.
Gate reference: `fcc09461cb0c0de4ef067c160f24d205f0f65c30`, not imported.
READY receipt: `msg_plugin-w_1791778252_5446dd`.
Complete package Round 189: `ea1b79c322d23d09219a006ab55a1ce319825ce0`, `verdicts/p3-worker.md`.
Package receipt: `msg_plugin-w_1791778514_13bd8f`.

I read the complete three-file repair, affected whole-change behavior, new tests, scripts/logs, body, and completed gate.
The repair has 169 additions and fourteen deletions.
Its full-index SHA256 is `b0e39c8a317ee5ce5ab0b55770c923d56a37aa1c867b71f86b1136b43ea16dce`.
The pending, real-pending, and held files are byte-identical to the reviewed parent.

### R302-SI-1 / F142 — HIGH — CLOSED

`receive_route` records each semantic frame's body length plus its four-byte prefix.
`Received::Input` carries that length to `enqueue_route_input`.
The queued input retains this charge; `route_bytes` sums it for the applicable route.
Bytes, Text, and Paste retain their existing payload-byte charge.
Key, Mouse, and Focus retain their stream-frame charge while queued.
The reader's undecoded bytes remain charged. Frames waiting for refusal room cause a zero read allowance.
Encoding still occurs at transaction start through the model. No early encoding or terminal byte constants enter the source.
Active input keeps the existing encoded-unwritten-byte charge.
Settlement before the next input starts can temporarily retain a conservative allowance; the next recomputation updates it.

The new behavior test blocks a preceding write and checks each queued key's reduced read allowance.
It observes insufficient room for another frame, then returned room after PTY progress.
It supplies another key when room returns and compares every resulting PTY byte with the model encoder's output, in order.
The test observes actions and bytes, without inspecting the private input queue or adding a production accessor.
Its normal PASS and its zero/one-charge mutation catches appear in the exact-head gate.

### R302-SI-2 / F144 — HIGH — CLOSED

`receive_route` supplies the configured `CoreLimits.max_key_text_bytes` to `Route::decide`.
The Key text check uses UTF-8 byte length before other key size computations or encoding.
An oversized text produces `TooLarge` with its optional route op and never enters PTY admission.
Existing before-live and view-only decisions remain first.

The new behavior test sets the bound to eight bytes.
It supplies nine bytes with op 5 and observes exactly one `TooLarge` refusal.
It supplies eight bytes and later Bytes input; the PTY contains only those admitted inputs.
This proves the boundary, zero effect of refusal, and continued route use.
The exact-head gate passes the test and catches the changed bound comparisons.

### R302-SI-3 / F143 — LOW — CLOSED

I read the complete current body, raw JSON SHA256 `d38dbd7c94ec34d40e9518cb234cdac0cef9a1b1e155dd973e4f7c0deea566e9`.
It now assigns testkit FAIL/no matching event and real INCONCLUSIVE/step_timeout to all fourteen focus trials.
Those classifications agree with the previously read raw outcomes.
The earlier corrections remain: reviewer 0165, four before identifiers, dp3 before not run, and the retained 8c local failure.
The c4 local rerun remains an unretained owner report. Historical source heads and gate references remain distinct.
The body describes both repairs, both new tests, and the exact repair-head gate.

### Evidence

I read the complete reversal script and log.
They substitute only the two production files from the reviewed parent while retaining the two new tests.
Both tests fail on the old source and pass after restoration of this exact head.
The byte-charge failure reports 1116 instead of 993. The bound failure reports no refusal instead of op 5 `TooLarge`.
Log SHA256: `1fe08fec51038bfa97d531902c0020c65dc7cacfac55b3fe5eef380732111ca0`.
The script has no clean marker or numeric Cargo test exits. It sets build jobs to four but not the nextest thread cap.
This record supplies supporting reversal outcomes, not independent cache proof.
The first four test reversals remain the earlier unretained owner report.

I read the complete local-check script and log.
The log names this exact head and the 04:02:01 UTC target clean with exit zero.
Nextest reports 1205 PASS and 429 skipped; nextest and prebuild exit zero.
Strict conformance reports 262 PASS, zero failures, and exit zero with seeds 0–31.
It runs 318 of 336 pending ids; eighteen are real-only and three are held.
Log SHA256: `eb401a2fe9c99c6d178bea5ca2c22edd6fb6c4a066832ef2e9e742512e0db9a5`.
The script sets both parallelism caps and removes the toolchain override.
No new pending removal follows from this repair. The .41 pin lacks an A15-1 transcript; the behavior test supplies its bound proof.

Gate: `~/botster-sessions/gates/botster-core-stage1-p3-route-semantic-input-78d7057f-pool-20261011-210519-99514.log`.
SHA256: `44f9d4f5868c05d969a651afd0644d81f2b783d41c827aa1c02a20cdd96824fd`.
All ten steps PASS in 203.4 seconds. Job and wrapper exit zero after 286 seconds on msa1.
The job has 71 queued seconds and 215 run seconds.
Default: 1871 PASS, 429 skipped, 20.386 seconds. Slow: 613 PASS, 2039 skipped, 47.013 seconds.
Both strict summaries report 262 PASS and zero failures.
Held-full runs two budget ids at 32 seeds: 64 expected ignored results, zero failures, 19.29 seconds.
The two new regressions and the earlier semantic-input tests have normal PASS records.
All three original both-tier removals retain normal PASS on both tiers. dp4 retains normal testkit PASS and its real-pending scope.
All 34 reported passing ignored real ids remain in the unchanged 45-entry real-pending list.

I independently match all 23 unique mutation outcomes: 22 caught, one unviable, zero missed, zero timeout.
All 22 raw catches equal their citations; every cited test has normal PASS.
Stage 2 does not run because stage 1 misses no mutation. The log has no FLAKY token.
The mutation outcomes take 25.5 seconds; the whole mutation step takes 28.5 seconds.
Fuzz selects no changed decoder harness.

### Disposition

All three integration findings close. The complete package verdict agrees.
The source preserves transaction-start encoding, optional route ops, DP-5 outcomes, and separate refusal pressure.
Resize remains PR B. DP-12 focus state and the historical fence/cache/evidence limits remain separate.
This review accepts the exact repair head and its completed evidence.
The final slot, current-v1 import/checks, applicable delta reviews, exact-final-head gate, final evidence reviews, and Q7 remain due.
No future source, import, pin, pending removal, gate, or merge is accepted.
I ran no build, test, gate, mutation, reversal, measurement, or executable base check.

VERDICT: CLEAN (0 open) at 78d7057fca48480e2e8e3174baa2e5c2ab7a70c3 against source base bfef88653bb59ad8d3cd38da1e206f7aba64ce92

## SLOT source inspection — complete package delta remains due

Inspected head: `953aa2ba300df85bff1df7a8be15f5214b6ca67b`.
Previous reviewed head: `78d7057fca48480e2e8e3174baa2e5c2ab7a70c3`.
Imported v1 base: `e37b91d6d4b5c98739f0f1ebc475a1e13d48d71c`.
Tree: `4871354dba1b1901c1a79d651e28ddd80426bcd2`.
Request: `msg_plugin-w_1791831176_173955`.
The exact pushed branch and remote v1 match these identities.
The commit has exactly those two parents. Their merge base is `bfef88653bb59ad8d3cd38da1e206f7aba64ce92`.
The owner reports one v1 merge with rerere disabled.

The reviewer reads the complete five-file own delta and the pending-list resolution through `git show --remerge-diff`.
Own full-index delta SHA-256: `cc8bc97f0b94a6d0ea47362669426348e63ea2a29eee5d1640c175a64acd73f2`.
The delta has 375 additions and 26 deletions.
Every own added and removed line equals the accepted repair's own delta, excluding indices and hunk positions.
No source conflict occurs.
The route implementation and route tests are byte-identical to the accepted repair.
The input implementation also imports v1's accepted removal of the worker grace state from `stopping`.
That input-file import equals the same change between the common base and v1.
No semantic input behavior changes in this import.

R302-SI-1 and R302-SI-2 stay closed in source.
Queued semantic inputs retain their stream-frame charge. Active inputs retain their existing encoded-unwritten charge.
The reader's pending bytes and refusal reservation still limit route reads.
The configured key-text bound still applies before other key size work and encoding.
The boundary and retained-byte regression tests remain exact.
Transaction-start model encoding, optional route ops, `NotReported`, and serialized `Unsupported` outcomes remain preserved.
The imported #305 no-grace stopping path remains intact.
The host driver, queue controls, operation holds, production worker, manifest, lockfile, known-flaky entry, and mutation configuration equal v1.
No new decoder, production test branch, or terminal encoder appears.
No new source finding is identified.

All nine conformance text sets satisfy the three-way formula.
The conflict removes `in_1_host_write_and_route_frames_share_payload_kinds` through #302 and both `in_6` ids through v1.
The resolution returns none of these ids to pending.
Against v1, the accepted three both-tier removals remain removed.
`dp_4_worker_encodes_route_frames_with_write_time_modes` still moves from testkit pending to real-pending.
No other identifier changes against v1.
Counts: pending 399, real-pending 35, held four, ledger 691, minimum 69, real-only 26, real-proofs seven, deferred three, withdrawn 27.
The `.44` pin and accepted base interfaces remain unchanged.
No new removal follows from the local record or this inspection.

### Body and supporting local record

The reviewer reads the complete current body.
Body SHA-256: `04bac9fd47d97cb59b234b8adf5aaea0a3c4d0565c0d50c5c69a9a4e12b825c4`.
The current-head section names the one list conflict, source preservation, local checks, and the lead's future train-7 gate.
The earlier `.41` records, before/after coverage, original test reversals, tier-specific outcomes, and unretained rerun keep their historical scope.
R302-SI-3 remains closed. No new body finding is identified.

Local log: `~/botster-sessions/shared/core-stage1/evidence/p3-302-merge-check-953aa2ba.log`.
SHA-256: `eff3505404bac6e15bd56cb5bd94093942fe85f7006715f6f0dc79a2bdea926c`.
The reviewer reads its producer script and audits the retained headers, test outcomes, summaries, and command exits.
The script removes the toolchain override and sets both concurrency caps to four.
It sets strict conformance and seeds 0–31 for the default-tier nextest conformance command.
The log identifies this exact head and target clean at `2026-10-12T18:43:17Z`, removing 25970 files and 6.7 GiB.
It reports zero exits for fmt, the taint CI step, lists, dependent test compilation, worker-core tests, and strict conformance.
Worker-core passes 199 tests with zero skips in 0.736 seconds.
Strict conformance passes 580 tests with 111 skips in 23.980 seconds.
The reviewer counts 779 unique PASS results across both runs; 312 conformance results are marked `[pending: strict]`.
The log reports no runtime FAIL, FLAKY, RETRY, TIMEOUT, or INCONCLUSIVE result.
The three both-tier removals, dp4 testkit trial, and repaired key-text test have retained local PASS records.
The script prints no numeric clean exit and provides no independent target-writer history.
This local default-tier record supplies no current-head real-tier, mutation, or full-gate result.

### Remaining conditions

The complete matching package delta remains due. This inspection completes no CLEAN round.
The lead owns train 7 and its gate.
Train composition reviews, the completed exact-train-head gate, final bodies, matching final receipts, and Q7 remain required.
The earlier gates are not relabeled as executions of this SLOT head.
Resize, DP-12 focus state, the real dp4 fence, and other separately owned gaps remain outside this source review.
The reviewer runs no build, test, gate, mutation, reversal, reproduction, measurement, or executable base check.
Wait for the complete package verdict doorbell. Never poll.
