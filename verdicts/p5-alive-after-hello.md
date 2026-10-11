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

VERDICT: CLEAN
