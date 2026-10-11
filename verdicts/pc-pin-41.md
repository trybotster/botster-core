# PR #289: contracts-v0.1.39 to contracts-v0.1.41

VERDICT: NOT CLEAN (2 open).

READY: `msg_plugin-w_1791755720_3b91b3`.
Head: `4e26982b4c8ed836e43baa30f18fbf8134db90d5`.
Original branch base: `23550811fb84c3c2cb066019716987575d7aecb2`.
Current PR, own-diff, and gate base: `f68f26a7b12738f2b19ab39fe76a93f3d1a72f12`.
The lead approves one early base import after the stale-base lists failure.
Merge `d82ec32f6e22584f9f040b170355de5c20afb4f0` has parents `bd5ea95b838aa07bbda980b71368817f7ba4435c` and the current base.
The later head changes only four pending-file transfers after that import.
Risk HIGH is correct: this PR moves a contracts pin and changes the shared testkit.

## Scope and source review

The reviewer reads the binding brief first and follows its established reading order.
BUILD.md, the current plan rules, the lead's PART B decision, R-73, R-74, the control rows, and erratum 6 govern this review.
The lead's `.41` decision makes one direct `.39` to `.41` move after #282.
The reviewer reads the complete own diff across all seven paths, control callers, request/acknowledgment code, report handling, and changed tests.
The reviewer reads the live PR body and verifies its exact head and base.

- All seven direct contracts dependencies move to `.41`. The nine lockfile source changes include the two transitive contract crates.
- The lockfile changes no package version, checksum, registry dependency, or unrelated source.
- The old tag resolves to `ddce507e2c08e740f899e97dcfec566f3d2275b8`; the new tag resolves to `2fcd7739708fe6de1390ff9bd9f90da5a8bc6d87`.
- The body lists `.40` PRs #51, #52, #53, and #55, then `.41` PRs #56 to #59 and the ruling text.
- The body correctly excludes #60 and #63 from `.41`.
- The unchanged `ci/remote/job-common.sh` equals both tags byte for byte. SHA-256: `ab191b18e47e04d05b7f3946bfe4c02c1e645a27117fb2766711693f184ecaaf`.
- The Core ledger equals all 691 Core ids in the `.41` ledger. Its only new id is erratum 6's `e6_1`.
- The new id remains pending with its observed second-route query failure and RC ownership. This PR claims no erratum-6 implementation.

The real `pty_queued` control sends Request::Queued through the existing bounded request path.
The probe reads FIONREAD on stdin and returns Report::Queued instead of Ack. It changes no probe gate state.
The server records the count and advances the acknowledgment count once. It notifies the control waiter but sends no host wake for this report.
The control returns the probe's count for the current worker identity. It reads no terminal model and pumps no Core state.
The harness's mutable control entry serializes these requests; the existing request path retains its cleanup bound.
The parser denies unknown fields. `kernel_bytes` is ignored on the real tier and is rejected with `on: false` before any worker request.
The testkit half remains the separately merged #283 implementation. No production worker, terminal, clock, or timeout code changes here.

The changed route-log test follows R-73's transcript-frame attribution.
It preserves the literal case, accepts the op that sent the bytes, and rejects an op that sent none and the unattributed-without-op case.
No frozen fake or transcript is changed by this Core PR. The required Prior art note is present.
No new source logic finding is identified in this round.

## Pending accounting

The reviewer independently compares both pending files and checks the pinned fence-exception file.

`core-pending.txt` removes `am_2_one_admission_point_orders_all_sources`, `am_2_transaction_is_contiguous_across_short_writes`, and `th_3_no_route_io_in_the_host`.
It also transfers these four ids to `core-real-pending.txt`:

- `a2_2_refused_is_a_sequencing_error_route_input_only`;
- `dp_3_data_frames_are_raw`;
- `dp_5_abort_before_commit_writes_nothing_and_sends_no_frame`;
- `dp_5_paste_is_assembled_and_written_contiguously_at_commit`.

Each transfer has one pinned `.41` fence exception. The allowed entry point is the same PR's testkit removal.
The new `e6_1_reply_from_another_route_is_wrong_route_and_the_query_stays_open` enters the ledger and testkit pending file together.

The real-pending removals are:

- `ev_2_drop_oldest_with_marker`;
- `ev_5_released_retires_keyed_droppable_and_loss_events`;
- `in_2_link_loss_is_unknown_not_zero`;
- `in_7_lost_worker_ends_pending`.

Each has no pinned fence exception and a selected PASS in the current head's default and real slow runs.
The earlier gate also reports those four real-pending trials PASS. That establishes the observed runs, not the required repeated record.
Other reported pending-real passes remain pending with the body’s named fence or failure record. No additional removal is accepted here.

## PIN41-1 — MEDIUM: the A5-4 before/after table is missing

The pin changes contract transcripts, relation matching, held-write controls, and the active ledger.
The lead's PART B decision requires the A5-4 comparison against `.39`, on both tiers, with no PASS-to-non-PASS result.
READY and the body supply gates and selected result counts but no complete comparison table or its raw evidence.
PC confirms that no such table exists in `msg_plugin-w_1791755816_ab4038`.

Required change: publish the full before/after table and raw evidence, then link them from the body.
Use the same Core base for the `.39` comparison and identify the exact before and after source revisions.
Show every Core id on both tiers, include the new id, and explain every changed result.
Keep previously invalid B13 evidence excluded. A planned or running evidence job does not close this finding.

## PIN41-2 — MEDIUM: the required N-at-least-five real proof is missing

The lead's 04:56 PART B decision requires a Linux gate and N of at least five for the six held-write ids.
The two held-write removals, `in_2_link_loss_is_unknown_not_zero` and `in_7_lost_worker_ends_pending`, have gate PASS records but no repeated record.
PC confirms that no repeated real record exists in `msg_plugin-w_1791755816_ab4038`.
The earlier shared-target evidence was withdrawn under B13 and cannot supply these runs.

Required change: publish the required repeated real proof for the six held-write ids and link its source, raw runs, and outcomes in the body.
PC plans five runs of the four removals and the other four pty_queued users, with each tier identified.
That planned set also covers the two event-fence removals. This finding adds no separate N requirement for those two ids.
Retain the two non-removal held-write cases in the real-pending file while their `.41` fence exceptions remain.
Use pool evidence or a proven clean target history. Do not count a single gate PASS as the complete repeated proof.
This finding concerns the named PART B evidence requirement. It adds no new test count or rerun rule.

## Supplied gates

Current log: `~/botster-sessions/gates/botster-core-stage1-pc-pin-41-4e26982b-pool-20261011-144733-21583.log`.
SHA-256: `426854307dda1a64cf844de2018c61daa3dcaad3cd6c303a3b71af3790e1169e`.
The reviewer reads the exact-head header, summaries, exits, selected pending outcomes, and mutation records.
All ten steps PASS in 404.4 seconds. The Linux job and wrapper exit 0 after 420 seconds on msa1.
Default: 1786 passed, 461 skipped, 13.201 seconds. Slow: 586 passed, 2017 skipped, 42.252 seconds.
Conformance: 230 passed, zero failures. Testkit minimum 64/69; real passing 56/68, real accepted 57/69.
The real pending report has 26 trials and 15 observed passes. This verdict accepts no extra removal from that report.

Stage 1 tests eight mutants and misses all eight. Stage 2 catches all eight.
The eight raw caught identities match the eight named catchers, without an unmatched identity.
The `pty_queued` match-arm deletion is caught by `conf::in_2_link_loss_is_unknown_not_zero`.
The Report::Queued acknowledgment mutants are caught by that same real conformance test.
Final mutation result: zero missed, timeout, or unviable. The log contains no FLAKY line.
Fuzz PASS reports no changed decoder harness; it claims no new decoder trial.

The reviewer also reads both earlier failed gate summaries and the relevant failure records:

- `ea67844e42a9ebe0ee4cc414fde9255a84614238`: RED at mutants; the pty_queued arm survives both stages; fuzz NOT RUN.
  Log SHA-256: `3b73535e4c56ed8002dea1ef1ada981af2be71cbf72478ad7c1a0afae1eedea1`.
  The served-controls test now names pty_queued. The current gate catches that same deleted arm.
- `bd5ea95b838aa07bbda980b71368817f7ba4435c`: RED at lists after the base removes seven pending ids.
  Log SHA-256: `62fac40202d61b7bde456e95b86a5193594373b202cc02d109792a7b1e18ffd8`.
  Its six later steps, including mutants, are NOT RUN. The lead-approved import addresses that base mismatch.

Both red gates remain RED and are retained in the body. The current green gate does not supply the two missing evidence records.
The manually reported arm reversal has not been read. The exact-head mutation result already supplies the selected arm-deletion catch.
The reported local strict counts and clean history are not independent reviewer execution.

## Next conditions

PC plans two separate pool evidence jobs, one at a time, and will send the complete records after the doorbell.
Both evidence findings must close before CLEAN. The HIGH PR also needs Sol's same-head source verdict.
The current final-slot queue gives #289 a later slot. No final import, final gate, Q7, or merge is approved here.
The reviewer runs no build, test, gate, reversal, mutant, or executable base check. Never poll.

## Sol Round 1 receipt

Receipt: `msg_plugin-w_1791756279_bad220`.
Sol verdict: `45307d6fe71f09a1e5302d45dfd25c488d20d804`, `verdicts/stage1/pc-pin-41.md`, Round 1.
The reviewer reads the complete verdict and verifies publication on `origin/stage1/integration-review-sol`.
Both reviews cover head `4e26982b4c8ed836e43baa30f18fbf8134db90d5` and base `f68f26a7b12738f2b19ab39fe76a93f3d1a72f12`.
P289-SI-1 matches PIN41-1. P289-SI-2 matches PIN41-2, including the six-id scope and no added event-fence repetition requirement.
Sol identifies no new integration source defect.
Sol independently verifies the pin, ledger, control boundary, pending accounting, and completed gate.
Its eight raw catches, Stage 1 misses, and named citations match uniquely. Each named catcher has a normal PASS.
The two evidence findings remain open. This receipt closes neither finding and accepts no running evidence job.
The final-slot and merge conditions remain due. No execution job runs here. Never poll.

## Round 2: completed evidence and IN-7 restoration

VERDICT: CLEAN. No open package finding.

PC receipts: `msg_plugin-w_1791756328_044342`, `msg_plugin-w_1791756384_01c2e7`, and `msg_plugin-w_1791756574_615bb6`.
New head: `f45794a454761c20fa2cfb9c0bd1b0f9fabecfbc`.
Parent: the reviewed and gated `4e26982b4c8ed836e43baa30f18fbf8134db90d5`.
Tree: `6ed07fea1e12fba1d4efd83112cc95b825abea00`.
Base remains `f68f26a7b12738f2b19ab39fe76a93f3d1a72f12`.
The reviewer rereads the binding brief before the delta review and verifies the new live head and body.
The complete delta restores only `in_7_lost_worker_ends_pending` to `core-real-pending.txt`.
The implementation, pin, lock, tests, ledger, and other pending entries remain unchanged.
This restoration is within the unmerged PR. It does not add an id that the merged base had already removed.
The PR now removes three real-pending ids. IN-7 stays pending and claims no five-PASS proof.

### PIN41-1 closure: complete table and accepted baseline failure

Evidence directory: `~/botster-sessions/shared/core-stage1/evidence/pc-pin-41/`.
The reviewer reads both job scripts, both table parsers, the raw job headers and results, and the complete table data.
Both raw jobs use Linux `6.12.111+deb13-amd64` on msa1.
Before source: base `f68f26a7`, pinned `.39`. After source: parent `4e26982b`, pinned `.41`.
The reviewer accepts these as pool artifacts from those recorded sources, not independent reviewer execution.

| Artifact | SHA-256 |
| --- | --- |
| `a54-linux-before-f68f26a7.log` | `7159ba5eedff88e920b4959c44058d39ca60e9be9e769f702501ea41806bed32` |
| `a54-nrun-linux-after-4e26982b.log` | `e4358f490347669aeee40f4aa0229ecaac8dddda7c221b0d343a81f8b7344788` |
| `a54-both.log` | `32b237906315be439f62bddb3eee23b854bf7f67d9db0b9dbbb1738d827c2b7e` |
| `a54-testkit-v039-v041.md` (v2) | `cef1931879478b9c405310db42ff6375aa73924e0ea07d77f0c58b4675f8da5d` |
| `a54-real-v039-v041.md` (v2) | `d4ebfc8cd1d550addc2545dbc82ae7ab27374c52b4abce77bcb9aa9a204221bd` |
| `nrun-real-testkit-4e26982b.md` | `85c8154436669c6f688c2ddf9447d99e6c2f4ba58f949cc63875297b2a773596` |

The first parser omitted 93 testkit and 92 real ledger rows with other markers.
PC preserves that parser as v1 and regenerates both tables with v2, without another run.
The revised tables include no-transcript, withdrawn, deferred, real-only, and testkit-proven reasons.
The reviewer independently matches every table cell with its raw result and detail. Both tables have 691 rows and zero mismatches.
The new E6-1 row is absent before and pending after. Omitted-category restoration changes no result.

Testkit: 223 must-pass PASS before, 230 after, eight changed rows, no PASS-to-non-PASS.
Real: 198 must-pass PASS before, 204 after; pending-real PASS four before, fourteen after; 21 changed rows.
The real after run also records one must-pass FAIL: `ev_5_route_closed_retires_route_keyed_events`, seed 7, step 28.
It receives `activity_before_close: false, closed: true` instead of both true.
The table retains that PASS-to-FAIL. It does not claim an all-green comparison.
The scripts print outcomes through pipelines and finish after all records. Their wrapper exit 0 is not a test PASS.
The before job exits 0 after 79 seconds; the after job exits 0 after 67 seconds. The after real test itself reports FAILED.

Lead disposition: `msg_plugin-w_1791756534_fb4088` accepts this documented failure for #289 and says it does not block CLEAN.
The lead supplies an accepted 23bl(A) controlled reproduction on the same `.39` base.
Log: `~/botster-sessions/gates/botster-core-detached-f68f26a7-f68f26a7-pool-20261011-150206-64641.log`.
SHA-256: `b0fa78606981a6fe0621558c35d473d7c0aac59452fa402790443904caedcc8c`.
Scratch diff SHA-256: `027d7d7aad29ad7cd084dd9081f76001d837a3d07671debae1ac858e339f5397`.
Snapshot: `9c44c653e9d71c32f276842b870a4c6a8a8dc203`, marked `--allow-dirty` and not a merge gate.

The reviewer reads the header, complete scratch diff, exact failure, trace, exits, and unmodified worker-loop order.
The diagnostic delays the selected second-route input for one second and preserves its bytes while the control link continues.
The worker loop handles control before route input. Detach can therefore close the route before that delayed input reaches the machine.
The test fails at seed 0, step 28, with the exact expected and received values above. It exits 101.
The trace shows a held 14-byte transport read ending in `0x63` and a route close in the same millisecond.
The failure precedes release of the delayed input. This proves the observed prefix, not later release behavior.
The shell prints the trace and exits 0. That wrapper result does not change the failure.

The reviewer reads package acceptance `1ad1761f78871340bc7745580755080544203d8e` and Sol acceptance `acd9d7f5809f83e6c5da91be89ddc0e722675d05`.
Both retain the controlled-diagnostic limits. The lead explicitly applies that baseline classification to #289.
This review claims no uninstrumented `.39` failure and no trace from PC's original after-run failure.
The transcript is byte-identical across `.39` and `.41`. It uses no held-write control, and its route-reports path is unchanged here.
The core-conformance runtime source delta adds only the fill constant; generic runner source is unchanged.
The source checks support non-interference on the observed path. The controlled baseline and lead disposition supply the accepted attribution.

The repair is RC's contracts #69, R-66 follow-up, into `.42`. #289 does not wait for that fix.
The observed failure stays in this verdict and the body. No new pending entry or rerun-to-PASS is permitted.
PIN41-1 closes with these explicit limits.

### PIN41-2 closure: five distinct runs and preserved IN-7 failure

The reviewer independently parses all 80 distinct N-run records: eight ids, two tiers, five numbered runs each.
No record is duplicated or missing. The six held-write ids and two additional event-fence ids are identified in the table.
All testkit pairs pass five of five. Seven real pairs pass five of five.
IN-7 real passes four of five. Run 4 is INCONCLUSIVE at seed 3, step 7, with `step_timeout`.
The complete failed trial remains in the raw log and table. It is not replaced by a successful rerun.
PC restores IN-7 to its original real-pending status. The three remaining real removals have the required valid evidence for their scope.
The other held-write ids keep their existing fence-exception and pending limits despite the observed five passes.
PIN41-2 closes as an evidence condition. The record accepts no claim that IN-7 now meets real acceptance.

### Arm reversal and static-head limits

The reviewer reads `reversal-pty-queued-arm-4e26982b.log`.
SHA-256: `99ef8572cac4dec45e003ed5056aae80c27866128b89e90bbe3e49c9418c5bfd`.
The scratch diff deletes only the pty_queued match arm. The served-controls test reaches `pty_queued: Err(Unsupported)` and exits 101.
After restoration, the same test passes and exits 0. Both runs compile the PR's testkit crate.
PC reports that the run follows a clean. The reviewer does not independently verify the clean or the shared-target writer history.
The separately reviewed exact-head pool mutation catch remains valid independently of that local history.

Static log for the list-only head: `static-f45794a4.log`.
SHA-256: `12c4e2e1031e127b53ec690ad4a16250350ac914daf518692ecad5b4c638b55c`.
The reviewer reads the complete log. Taint and lists report success; the lists output names base f68f26a7 and 27 real-pending ids.
PC reports fmt success and static exit 0. The log contains no exact-head header; those run-to-head details remain PC's report.
The parent green gate remains evidence for the unchanged implementation, not a full gate on f45794a4.
The lead explicitly requires no separate full gate now. The final SLOT gate must cover the final head.

Both package findings close on f45794a4. Sol's matching new-head evidence verdict remains due here.
The final slot, applicable import/check/review, exact-head final green gate, completed body, and Q7 remain required.
No current merge approval is given. The reviewer runs no executable job or base check. Never poll.
