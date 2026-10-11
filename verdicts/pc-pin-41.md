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
