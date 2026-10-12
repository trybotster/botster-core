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

## Sol Round 2 CLEAN receipt and evidence scope

Receipt: `msg_plugin-w_1791757363_d2ecc6`.
Sol verdict: `3320652a81221ad3e01f936195ffcb03f2abffc1`, `verdicts/stage1/pc-pin-41.md`, Round 2.
The reviewer reads the complete new round and verifies publication on `origin/stage1/integration-review-sol`.
Both source/evidence CLEANs cover `f45794a454761c20fa2cfb9c0bd1b0f9fabecfbc` and base `f68f26a7b12738f2b19ab39fe76a93f3d1a72f12`.
Sol independently closes both evidence findings and retains the IN-7 failure, controlled-baseline limits, and final-slot conditions.
The matching integration source/evidence receipt condition is closed.

The jobs leave BOTSTER_SEEDS unset. The reviewed library selects its default local seeds 0–7.
The current body explicitly states that scope. The comparison and N-run evidence claim no seeds 0–31 or Mac execution.
The earlier gate retains its separate CI scope. The evidence jobs do not replace that gate or the required final gate.
The reviewer verifies the live body on this unchanged head and base.
Body SHA-256: `c70e93fae81bf832f3e26cb5ffa91f0319f5025b84b8c5961dd6403be2e76dfa`.

The reviewer reads the complete IN-7 rule-9 record:
`~/botster-sessions/shared/core-stage1/handoffs/flaky-in-7-lost-worker-ends-pending-4e26982b.md`.
SHA-256: `0f92c5759c624a1a9a5bc85b947ffa42e9567a7e21920f8d897bc8b957d21f29`.
The failed trial stops at the R-74 pty_queued fence, seed 3 step 7. Its diagnosis remains open, owner PC, after #289 and R-78.
The record accepts no R-88 attribution, successful replacement run, or real acceptance for that id.

Both source/evidence reviews are CLEAN. The final SLOT import/check/review, exact-head final green gate, body, and Q7 remain due.
The parent gate remains a parent gate. No final merge approval is given. No execution job runs here. Never poll.

## Round 3: final-slot source delta

VERDICT: CLEAN for the source delta (0 open source findings). Final evidence approval remains due.

Request: `msg_plugin-w_1791765546_144247`.
Reviewed head: `0abf666c001e7dcd55e3bc86f0a95e168c183f26`.
Parents: reviewed `f45794a454761c20fa2cfb9c0bd1b0f9fabecfbc` and v1 `b0c0a419af27259006b5e06d4e5da3b3d00083e6`.
Their common ancestor is `f68f26a7b12738f2b19ab39fe76a93f3d1a72f12`.
The reviewer reads the brief first, the complete own diff, the overlapping import, the request callers, and both merge records.
The prior full review covers the unchanged pin, lockfile, control implementation, tests, and retained evidence.
The completed `base-merge-check` is FAIL and correctly requires both delta reviews.
The reviewer runs no executable base check or other execution job.

The real program module imports v1's `Requests` wrapper and shared output path.
The new `queued` method stays in `Programs` and calls its request method, which delegates to `Requests::request`.
It reads the report count from `self.requests.hub` after the request returns.
The same hub holds the requests, reports, counts, wake registrations, and cleanup state.
The probe `queued` field, Report::Queued handling, acknowledgement increment, served control, parser, and tests retain their reviewed behavior.
The imported real `route_fill` calls the same output request synchronously. It adds no background request writer in this path.
The merge changes no query count semantics, kernel buffering rule, worker identity check, or request deadline.
No additional source finding arises from the overlap.

The reviewer independently compares the four line-set files with the common ancestor and both parents.
Every merged id set equals both branches' additions minus both branches' removals.
There is no missing, extra, or re-added id.

| File | Common ancestor | Reviewed parent | v1 parent | Merged |
| --- | ---: | ---: | ---: | ---: |
| core-ledger-ids.txt | 690 | 691 | 690 | 691 |
| core-pending.txt | 444 | 438 | 422 | 416 |
| core-real-pending.txt | 26 | 27 | 39 | 40 |
| core-held.txt | 3 | 3 | 3 | 3 |

The pending-file conflict removes both separately removed DP-5 ids.
The real-pending file keeps both branches' added blocks and preserves the reviewed IN-7 restoration.
The comment and blank-line differences explain the line-set check failures; they change no id disposition.
Cargo.toml, Cargo.lock, the ledger, and the route-log test match the reviewed parent's blobs exactly.
The complete own diff adds no unrelated production code, pin change, or pending removal.

Base-check log: `~/botster-sessions/shared/core-stage1/evidence/pc-pin-41/base-merge-check-0abf666c.log`.
SHA-256: `d8b9d74d2c862a58d39d63c997f8e021781a1830628fd49484cdfde8ad196041`.
Static log: `~/botster-sessions/shared/core-stage1/evidence/pc-pin-41/static-0abf666c.log`.
SHA-256: `e7233aafe11be1970ed0f04655253a1b91796bfba0d14513db2186577d0029ce`.
The static header names this exact head and clean worktree, then records Cargo clean removing 3.3 GiB.
Clippy and all ten named xtask checks report exit zero. PC reports fmt success; the retained log has no fmt result line.
The list check names the v1 parent as its base and merge base, with 691 ledger ids and 40 real-pending ids.
These static records do not replace the exact-final-head gate.

The final strict result, one exact-head final gate, updated body, Sol's delta review, and Q7 remain due.
The earlier IN-7 and EV-5 evidence limits remain binding. This source verdict approves no final merge.
The reviewer waits for the evidence doorbell and does not poll.

## Round 4: four testkit removals remain real-pending

VERDICT: CLEAN for the source delta (0 open source findings). Final evidence approval remains due.

Request: `msg_plugin-w_1791765735_661a09`.
Head: `69d414995408a21a32b73d01608a798202057462`. Base remains `b0c0a419af27259006b5e06d4e5da3b3d00083e6`.
The reviewer reads the brief first and checks the complete delta from source-reviewed 0abf666c.
Two commits change only the two pending files. No implementation, test, pin, ledger, or held list changes.
The first commit removes four ids from testkit pending. The second places the same four ids in real-pending with an explanation.
The reviewer independently verifies that the removed and added sets are identical:

- `conf::dp_5_confirmed_paste_resend_is_a_new_op_with_require_bracketed_false`;
- `conf::dp_5_input_done_only_for_want_result_true`;
- `conf::dp_5_mode_precondition_zero_effect_reported_on_the_route`;
- `conf::in_9_kitty_unknown_flag_bits_ignored`.

The reviewer reads each exception in pinned contracts-v0.1.41.
The three DP-5 ids lack program_ready at steps 11, 13, and 11 respectively under R-60.
The IN-9 id lacks program_ready at step 11 and capture fences at steps 13, 19, and 25 under R-56.
Each transfer therefore meets the real-tier entry rule in the same PR's testkit removal.
The added block names P3 as the fence owner. No real PASS or fence completion is claimed.
The current files contain 412 testkit-pending and 44 real-pending ids, with no duplicate transfer.

The supporting strict log names 0abf666c plus the uncommitted four removals.
It reports the same worktree and target as static-0abf666c after its recorded clean.
Its header states `BOTSTER_PENDING_STRICT=1`, `BOTSTER_SEEDS=0-31`, and `--no-fail-fast`.
All four named tests have normal `ok` results. All suite summaries report zero failures and total 1178 passed.
The record ends with exit 0.
It supports the testkit removals, but it is not an exact-head final gate on 69d41499.
The earlier strict failure remains a failure on the still-pended merged tree; it does not establish a plain-v1 failure.
Pending-list presence on either parent does not prove that parent's test result.

Evidence files are in `~/botster-sessions/shared/core-stage1/evidence/pc-pin-41/`:

- `local-check-0abf666c-unpend4.log`: `9dbf93967357c5770e6f2d6ab5e1d0ba79819b0a0c74a946ef0a8a32509c72a4`.
- `lists-unpend4.log`: `eac2d9c7c6e7cea109bd85ba1edb35a78bd61d384b05d482971c88f5d169db52`.
- `lists-taint-realpend4.log`: `c53e6f0068e8a9d8c88c8959ea0304abe714db616b85345af1877a4620db2868`.

The last record labels the tree as 8ab0f6d5 plus the real-pending block, before its commit.
Lists and taint exit zero. The list result names 691 ledger ids, 412 testkit-pending ids, and 44 real-pending ids.
PC reports stopping the 8ab0f6d5 gate before execution. That report supplies no completed gate result.
The one final gate on 69d41499 is still running. A running gate does not supply approval.
The current body correction, completed exact-head gate, Sol's matching delta review, and Q7 remain due.
The earlier source and evidence limits remain binding. No final merge is approved. No reviewer execution job runs. Never poll.

### Round 4 matching integration receipt and current body

Sol receipt: `msg_plugin-w_1791765902_7ad08f`.
Published integration verdict: `2cd7aae57da51ed95f58f23097a49124207e0c60`, `verdicts/stage1/pc-pin-41.md`, Round 3.
The package reviewer reads the complete round and verifies publication on `origin/stage1/integration-review-sol`.
Both source delta CLEANs cover 69d41499 and base b0c0a419, including the four transfers.
Sol preserves the same source, list, evidence, and plain-v1 limits.

The package reviewer reads the complete current body through `gh`, the authorized GitHub fallback.
GitHub reports exact head 69d414995408a21a32b73d01608a798202057462 and base b0c0a419af27259006b5e06d4e5da3b3d00083e6.
The final-slot section names both identities, three conflicts, two list-only commits, current counts, and old evidence heads.
The body preserves the earlier EV-5 failure and IN-7 trial. It claims no plain-v1 result for the four new removals.
The body correctly marks the final gate RUNNING. It still awaits the completed gate and final review receipts.
The stale body metadata identified by Sol as P289-SI-3 is corrected in this read.
Sol records body SHA-256 `97ea97bf34f8f82aa2b66924619cf25e87748534644bdd556b4eef4d3cdb319d`; the package reviewer does not independently hash this read.
The completed final gate must report all eleven own testkit removals as normal PASS.
Final evidence approval and Q7 remain due. No running output is read. Never poll.

## Round 5: completed final gate and body

VERDICT: CLEAN (0 open package findings)

Head: `69d414995408a21a32b73d01608a798202057462`.
Base: `b0c0a419af27259006b5e06d4e5da3b3d00083e6`.
Completed evidence receipt: `msg_plugin-w_1791766453_57d995`.
The prior complete source delta reviews cover this exact head. This round checks the completed evidence and final body.
The reviewer runs no build, test, gate, mutant, reproduction, or executable base check.

Final gate: `~/botster-sessions/gates/botster-core-stage1-pc-pin-41-69d41499-pool-20261011-174215-53790.log`.
SHA-256: `5fb6af739f3bcc2cc75a876d6c8f96706eab3601c18da47116585d01ce202c2a`.
The header names this exact head and base. The Linux job runs on msa1 with four CPUs.
All ten steps PASS in 601.2 seconds. The job exits zero after 651 seconds; the gate wrapper exits zero after 652 seconds.
Default: 1843 passed, 435 skipped. Slow: 606 passed, 2040 skipped.
The unmutated slow baseline reports 606 passed, ten slow, and 2746 skipped.
The list step names 691 ledger ids, 412 testkit-pending ids, and 44 real-pending ids.
The conformance summary reports 256 passed and zero failures.

The reviewer derives all eleven own testkit removals from the base-to-head id sets.
Each removal has one normal PASS record in `botster-core::conformance`.
All three own real-pending removals have normal PASS records in `botster-core::slow_conformance` and in the unmutated baseline.
The accepted five-run proof remains tied to its earlier source. This gate supplies exact-final-head validation, not a new five-run record.
The real-pending report observes 32 passing trials. All 32 remain in the current real-pending set.
The eight own additions retain their pinned fence exceptions. The other retained ids keep their earlier fence or failure limits.
This gate makes no extra pending removal. IN-7 remains real-pending despite this passing trial.

Stage 1 tests eight mutants: zero caught, eight missed, zero timeout, zero unviable.
Stage 2 tests those eight and catches all eight: zero missed, timeout, or unviable.
The reviewer independently matches all eight unique raw catches to all eight named citations.
Each cited catcher has a normal PASS in the slow run and its unmutated baseline.
The log contains no FLAKY line. Fuzz PASS reports no changed decoder harness.

The reviewer reads the complete final body and hashes it independently.
Body SHA-256: `b3364d02e8d0d151c4c06b1e714849c2ddb92bada2e978f85cf85b152fa56c90`.
GitHub reports the exact reviewed head and base.
The body replaces RUNNING with the completed ten-step gate and names all eleven normal testkit PASS records.
It preserves the conflict resolutions, list transfers, older evidence heads, red gates, IN-7 failure, and accepted EV-5 attribution.
It claims no plain-v1 result for the four newer removals and no acceptance for their real tier.
The prior A5-4 and N-run records retain their earlier source and seed scope. This final gate does not erase their failed trials.

At the final check, remote v1 is b0c0a419 and the delivery branch is 69d41499.
The retained base-merge-check remains FAIL. Both required source delta rounds explicitly review its conflicts and later list changes.
The exact-final-head package source, evidence, and body are CLEAN.
Sol's final evidence receipt and the lead's Q7 check before merge remain due.
No later source head is accepted by this verdict. The reviewer waits for the doorbell and does not poll.
