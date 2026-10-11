# Package review: #249, payload_alive after worker evidence

## Round 1 — 2026-10-11

- Head: `c0694c7e295c9132d4b607fa8caee53e41a0466c`.
- Tree: `ad55bef8d84e46f0dffde1e050f0c5b7fe1e3418`.
- Base: `5c480db457797fa7ebecb33835a52e26a5a8490d`.
- READY: `msg_plugin-w_1791706704_8d6a7f`.
- Risk: HIGH, BUILD.md rule 3. This PR changes the shared testkit crate.

### AH-F1 MEDIUM — OPEN on this head

The strengthened hold test checks `live_children` before the tap has read the worker's hello.
The recorded process can still run `wrap`, which owns a live intermediate child.
That child is not a payload. The predicate returns true, and the next assertion reports that Launch passed the hold.

`Children::spawn` returns the process identity after `command.spawn`; it provides no exec barrier.
`pump_until_at` checks the predicate after a quiet pump. A quiet pump provides no exec or hello barrier.
The guard reaps its intermediate before it execs the worker. The PID and start time survive exec.
Thus this test can fail while the hold works correctly. The new control excludes this same process state with `heard`.

Required change: count a child as a leaked payload only after the tap has worker-exec evidence.
Keep the separate Launch-held observation. Keep the no-child assertion after that observation.
The package reviewer sent AH-F1 directly to P5 and Sol in `msg_plugin-w_1791706879_1315de` and `msg_plugin-w_1791706879_cf80d6`.
Sol independently identified the path. P5 agreed in `msg_plugin-w_1791706919_def42c`.

### Complete source review

The reviewer read all five own files, their complete delta, and the affected control, opener, guard, worker, and process paths.
The own delta has 203 insertions and 22 deletions.
`Heard` records instances for each data directory. The record survives tap drop and directory reopen.
An accepted link records the instance after a decoded hello. A successful `connect_worker` records the requested instance.
The worker sends hello or serves its endpoint after exec. Its guard has already reaped the intermediate at that point.
The worker creates its payload only after Launch.

For an unknown payload, `payload_alive` returns false when no tap has recorded that worker evidence.
This check reports a process fact. It introduces no wait, delay, or extra protocol step.
Known payload checks retain their exact process identity checks.
After worker evidence, the unknown-payload path retains child listing and the worker's subsequent identity check.
The shared record preserves the reopened-handle case even when the new tap has read no Launched report.
The new control applies to the short-root hold-test path named by the lead.

The new regression proof creates a shell with one live cat child.
A byte returned through a FIFO proves that the child runs.
The proof records the shell's matching identity, establishes no worker evidence, and observes public `payload_alive` as false.
The unit proof checks accepted hello, successful connection, absence before hello, and record retention after tap drop.
The real reopen proof retains live-payload, ended-payload, and ended-worker results.
The strengthened hold proof has AH-F1 on this head; its green run does not exclude that schedule.

The published body states HIGH and gives Prior art.
The old Core source at `72b2e3354ffc291e39f9a5d7eb2f9c5fcbb5e79c` has no corresponding child inference or start-hold mechanism.
The PR reuses hello decoding, the directory sharing pattern for Rows, Hold, and the existing process helpers.
Its separate record is necessary because a link and its tap can end before the worker ends.
The reviewer checked the published head, base, body, and evidence links.
No own dependency, contract pin, mutation exclusion, timeout, transcript, conformance list, or production Core source changes.
No debug marker remains in the final crate source.

### Retained execution evidence

The reviewer read supplied logs. The reviewer ran no build, test, gate, mutation, measurement, reversal, or executable merge check.

Pre-review Linux gate: `3406ad795b646cc6eea714b5d21a65db3f93483f`, base `475a442fa58cd2555c4121ce705ea3cf75470692`.
Log: `~/botster-sessions/gates/botster-core-stage1-p5-alive-after-hello-3406ad79-pool-20261011-004335-49218.log`.
All ten steps pass, total 1877.9 seconds. Job and gate exit zero after 1987 seconds on structvr-MS-7C77, with no queue wait.
Default tests: 1607 passed, 484 skipped, 16.564 seconds.
Slow tests: 428 passed, 1990 skipped, 260.326 seconds.
The actual real conformance records contain 128 distinct passes.
Testkit conformance passes 206. Strict pending runs 373/394, with 21 real-only and two held.
Real pending has 77 IDs and zero newly passing IDs. Core pending has 464 IDs.
The ledger has 690 IDs, two deferred IDs, and 18 withdrawn IDs.

The historical minimum report prints testkit 55/69, real 36/68, and accepted 37/69.
Approved plan 23ak applies R-59: AD-7 remains in its list but is an unfenced pass and does not count toward the real minimum.
The counted real minimum is 35/68, plus AD-7 as "passes, unfenced (R-59)".
This PR implements neither the contracts `start_held` control nor the nine transcript fences.
The reviewer makes no claim that this PR raises the accepted minimum.

Mutation stage 1 tests 15 mutants: eight caught, six missed, one unviable, zero timeout, 172.7 seconds.
The slow baseline passes 428 tests in 260.384 seconds; the full baseline step takes 277.3 seconds.
Stage 2 tests exactly those six miss identities once each: four caught, two unviable, zero missed or timeout, 779.4 seconds.
The reviewer checked every printed outcome and compared the full identity counts.
Stage 2 catches both `launch_held` constant mutants, the payload-result replacement, and deletion of the heard negation.
Default replacements for the harness and Core are unviable.
The final result is twelve caught, three unviable, zero missed or timeout. The mutation step takes 1232.6 seconds.

The retained base-merge check passes all four conditions at the Round 1 head.
Log: `~/botster-sessions/gates/botster-core-stage1-p5-alive-after-hello-c0694c7e-pool-20261011-011736-69133.log`.
The reviewer independently compared both complete own diffs with `--full-index`.
Their bytes are identical: 24831 bytes, SHA-256 `b7cc0dab62c2d722ca4d5f90061dd4a7913a4439ed11adabd6a10b760fb762b4`.
All four imported paths equal the current base bytes. Their paths do not overlap the five own paths.
Both pending files equal the current base. The check exits zero after eight seconds on msa1.

Focused Mac evidence uses `f07c3d1cdbd00b0b312ab503930c9aa52cdacfd4`.
Log: `~/botster-sessions/shared/core-stage1/gate-logs/hello-mac-f07c3d1c.log`.
The run passes 39/40 tests. The shell regression, hold, reopen, and hello proofs pass.
The existing zombie test expects Matches and receives Absent. Its test command exits 100.
This is catalog B9 and separate work. This review does not call the complete Mac run green.
Later own source changes only hold-test observations; the OS-decided control and shell proof retain their tested bytes.
The edge-tap source equals the Mac-tested source exactly.

The retained reversal removes only the five-line heard check at scratch head `8146eb6c2ed0d875657ced9a7ed443355357d093`.
Log: `~/botster-sessions/shared/core-stage1/evidence/p5-hello-revert-8146eb6c-linux.log`.
The new shell proof fails with alive true versus false. The test exits 100.
The shell wrapper prints that result and exits zero; its zero exit is not a passing test.

The reviewer also read retained root-evidence logs for both 30-run debug checks, the tap-order trace, and P3's process sampler.
Debug checks report 30/30 for the hold test and AD-7. Debug output changes timing and establishes no failure-rate cure.
The printed trace shows release before hello on the same tap address.
P3's sampler checks parents named botster-worker. It cannot observe the intermediate while its parent still runs wrap.
The source path and the synthetic regression establish the child-inference defect; sampler absence alone does not establish that cause.

Authority: approved plan 23ak, `~/botster-sessions/pins/stage1-plan.4eceef06.md`.
Verified SHA-256: `db7ba8e5c0748aa53257fb53a5e6f8923a9549c6c9871127a7d2ef2c97000813`.
BUILD.md remains binding at `56bd0a5`. This PR retains code pin contracts-v0.1.27 at `b161db87` before the separate pin move.
The lead's pool rule permits the pre-review gate and pure base import evidence, then requires the final gate after both CLEANs.

VERDICT: NOT CLEAN

## Round 2 — 2026-10-11

- Head: `0309469729459788da3a369bcab3443debc7a359`.
- Tree: `97bc42fce1cd3dc8d2cd84cb9b4fd789c49a4555`.
- Parent: `c0694c7e295c9132d4b607fa8caee53e41a0466c`.
- Base: `5c480db457797fa7ebecb33835a52e26a5a8490d`.
- Replacement submission: `msg_plugin-w_1791706971_e7b6a8`.
- Risk remains HIGH.

AH-F1 closes. The reviewer read the complete one-file correction: thirteen insertions and two deletions in the hold test.
The predicate still exits when Launch meets its hold. Before child listing, the predicate now requires `tap.heard(instance)`.
After that wait, the test asserts worker evidence and checks that the worker has no child.
It retains the initial negative held observation, the held-state control checks, release, and the subsequent live-payload result.
This change excludes wrap's intermediate from the leaked-payload inference and preserves the independent hold proof.
`session_processes` locks the directory's Rows record; it does not lock the tap again.
No source finding remains open, including LOW.

The reviewer checked the published replacement head, unchanged base, corrected body, parent, and tree.
All four imported paths still equal the current base. No conformance file changes. `git diff --check` reports no error.
The correction changes no production control or OS process operation. Round 1's source and evidence checks remain applicable.

Focused correction log: `~/botster-sessions/gates/botster-core-stage1-p5-alive-after-hello-03094697-pool-20261011-012153-14347.log`.
The header names this exact head and base. The prebuild passes, then testkit clippy with slow and all targets passes with `-D warnings`.
The selected hold and shell regression tests run together twenty times. The retained result is pass=20, fail=0.
The job exits zero after forty seconds on msa1, with no queue wait. The gate wrapper exits zero after forty-one seconds.
The wrapper's result is accepted with its explicit twenty-pass tally; the wrapper alone does not enforce a zero failure count.

P5 supplies a second reversal in `msg_plugin-w_1791707186_9a09f7`.
Scratch head `872b8aaf1724ffdfb2a6a70d4f371c67e3d5733d` adds only an early return that sends every nonempty buffer through the hold.
Log: `~/botster-sessions/gates/botster-core-scratch-p5-hold-leak-03094697-872b8aaf-pool-20261011-012416-33205.log`.
The reviewer read the scratch diff, exact log header, five failures, final tally, and published body update.
All five runs fail at the corrected no-child assertion with one live child: "the Launch passed the hold".
The tally is pass=0, fail=5. The wrapper exits zero because it prints the tally; those exits do not indicate passing tests.
This supplied reversal shows that the corrected proof detects a hold that sends Launch through.

Sol records Round 1 NOT CLEAN in `543d6d645e5749211b2c885bb92859a2dfed5412`.
The package reviewer read that complete integration artifact.
Sol's `msg_plugin-w_1791707043_fb2525` independently closes AH-F1 in source and awaits this completed package artifact.

The final gate on the reviewed head remains required after both CLEANs under the lead's pool rule.
This verdict closes no separate finding or gate for #233, #240, #243, or P6's short data root.
The package reviewer changed no product code and ran no execution job.

### Final-gate failure path — 2026-10-11

P5 requests the lead's condition-2 source check in `msg_plugin-w_1791707678_ad6708`.
The final gate on this exact head fails only real `conf::ev_5_repeated_attach_detach_without_polling_stays_bounded`, seed 0, step 21.
The result is PendingLimit, "pending_ops is 4". Mutants and fuzz do not run. The gate exits one.
Log: `~/botster-sessions/gates/botster-core-stage1-p5-alive-after-hello-03094697-pool-20261011-013016-71458.log`.
The supplied base trace at `5c480db457797fa7ebecb33835a52e26a5a8490d` also reports this ID failing at step 21, seed 1.
That trace uses the old contracts pin plus trace-only patches. Its retained excerpt does not print the received error.
Log: `~/botster-sessions/gates/botster-core-pc-ev5-trace-mac-old-pin-v1-5c480db4-20261011-011611.log`.

The reviewer read the complete pinned EV-5 transcript and the PendingLimit admission branch.
EV-5 starts its worker through Running before repeated attach and Detach operations.
It uses `edges_quiet`, but it uses neither `payload_alive` nor a start hold.
The shared tap executes the new Heard writes during startup. EV-5 does not read that record.
These writes change no returned bytes, descriptor operation, wake decision, or pending operation.
The transcript's later `edges_quiet` path retains the same read and wake decisions.
The new `launch_held` query runs only in tests. The changed real test does not run inside the EV-5 transcript.
All host, worker, worker-core, and real-route source bytes equal the base.

Condition-2 conclusion: #249's diff does not change EV-5's step-21 Detach admission or worker descriptor receipt path.
The reviewer does not claim that EV-5 executes none of the changed tap code.
The lead handoff approves one re-gate, subject to this written source check, and identifies #240 as the cause-removal PR.
The reviewer read proposed plan 23al at `2cb5eba4b93273a142a36adf91c04b883602be84` and the lead's written approval.
This source check does not certify an unread trace, make the red gate green, or remove the final-gate requirement.
The reviewer runs no re-gate. The implementer and lead retain that responsibility.

### Revised condition: diagnosed cause — 2026-10-11

Lead message `msg_plugin-w_1791708097_7ae086` replaces the path condition with a condition about the diagnosed cause.
Revised plan source: `1555a98b196c259dfe5f5d56a87c473ba49d49dd`.
The earlier addendum does not establish literal code-path exclusion. EV-5 executes the changed `LinkTap::observe` during hello receipt.
The earlier addendum explicitly records that overlap; its unchanged admission-path conclusion has this narrower scope.

The reviewer confirms that #249 does not alter the diagnosed missing-descriptor or unfenced-`poll_all` mechanisms.
The worker still reads control bytes without descriptor receipt. The worker's descriptor inputs and route actions remain unchanged.
The tap's existing decoder still reads the same bytes. `observe` adds only an instance insertion after the existing frame observation.
`link_recv` returns the same byte count and errors. `link_send` and `link_close` retain their exact base bytes.
The record supplies no instruction to close a link, change a returned byte, receive a descriptor, or complete a pending operation.
EV-5 never reads Heard, never calls `payload_alive`, and never arms a start hold.
Its `poll_all` and `await_quiet` steps retain their existing behavior and lack a worker-completion fence.

The extra record work can change elapsed time and scheduling. This review does not assert identical link-close times or failure frequencies.
That timing limit does not change the missing descriptor receipt or add the missing completion fence.
The source confirmation concerns those mechanisms; it does not independently certify the lead's complete trace diagnosis.
The lead handoff also records a correction: old-pin trace failures were UnknownRoute, while PendingLimit occurred on another traced head.
Thus the base trace excerpt does not establish an identical PendingLimit failure on plain v1.
The lead and integration reviewer must assess that trace evidence against the revised rule's same-failure and same-mechanism conditions.
This reviewer runs no gate and grants no exception beyond this source confirmation.

VERDICT: CLEAN

## Round 3 — final evidence, 2026-10-11

- Head: `0309469729459788da3a369bcab3443debc7a359`.
- Tree: `97bc42fce1cd3dc8d2cd84cb9b4fd789c49a4555`.
- Base: `5c480db457797fa7ebecb33835a52e26a5a8490d`.
- Implementation unchanged since Round 2. No source finding remains open, including LOW.
- Integration completion: `ef88d1a2c80147ac7b856fa3e2af2c9342b92dbc`, received in `msg_plugin-w_1791708707_f35602`.

The reviewer read Sol's committed completion section, the exact-head gate header, test results, mutation results, and complete final summary.
Final log: `~/botster-sessions/gates/botster-core-stage1-p5-alive-after-hello-03094697-pool-20261011-013626-2040.log`.
The header names this exact head and base. All ten steps pass, total 295.9 seconds.
The job exits zero after 303 seconds on msa1, with no queue wait. The gate wrapper exits zero after 304 seconds.
Default tests: 1611 passed, 484 skipped, 10.446 seconds.
Slow tests: 428 passed, 1994 skipped, 28.664 seconds.
EV-5 passes on the testkit and real tiers and in the unmutated slow baseline.
Testkit minimum remains 55/69. Real pending remains 77, with zero newly passing IDs.
The printed real 36/68 and accepted 37/69 remain historical raw counts; R-59's exclusion still applies to unfenced AD-7.

Stage 1 tests 15 mutants: eight caught, six missed, one unviable, zero timeout, 106.5 seconds.
The slow baseline passes all 428 tests in 29.155 seconds; the full baseline step takes 39.9 seconds.
Stage 2 tests exactly the six missed identities once each: four caught, two unviable, zero missed or timeout, 56.4 seconds.
The reviewer checked all 21 printed outcomes and the full identity counts.
The final result is twelve caught, three unviable, zero missed or timeout. The complete mutation step takes 204.8 seconds.
The fuzz step passes and reports no changed crate with a decoder harness.

Approved plan 23al is pinned at `~/botster-sessions/pins/stage1-plan.1555a98b.md`.
The pin and the approved source have matching SHA-256 `a3965b731b7b00fbe56e28581d298f38fe4d1b2836ff322f33d4c988cc5183ab`.
The lead handoff explicitly classifies the failure as the retry Detach's failed success expectation at the same step.
The lead connects four pending operations and zero queued events to P3's descriptor diagnosis and the R-63 recovery race.
The plain-v1 trace returns UnknownRoute; the first final gate returns PendingLimit.
This review preserves that distinction and claims no exact PendingLimit reproduction on plain v1.
Sol accepts the lead's explicit classification with that limit. This package record retains its qualified source confirmation.
The one authorized re-gate is now green on every step. This record grants no second re-gate.

The lead merged #249 at `b7c97b42b8e559053153f8a8202ee32dbde0a93a`.
The reviewer checked both parents: the exact base and reviewed head. The merge tree equals the reviewed and gated tree.
EV-5's R-63 fence and the R-64/A21 item remain open. This completion closes no separate gate or finding for another PR.
The reviewer changed no product code and ran no build, test, gate, mutation, reversal, measurement, or executable merge check.

VERDICT: CLEAN
