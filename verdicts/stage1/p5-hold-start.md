# Integration review — #301, testkit hold_start

## Round 1 — source, body, and completed first gate

PR: https://github.com/trybotster/botster-core/pull/301. HIGH under rule 3: shared testkit changes.
Head: `403bbf689cc9f745a9552cc518374a6a8f2cdb32`.
Base: `bfef88653bb59ad8d3cd38da1e206f7aba64ce92`.
Parent: `ae3fbfff24b9b5ced6aa7a53bbd06c394c603ef2`.
Tree: `2cc7ab25a4fd770876ae1d0e6e49fb9e9a0abcd5`.
READY: `msg_plugin-w_1791774608_fc8352`.
Sol independently verifies the merge base, parent, tree, complete nine-file diff, and unchanged real-pending set.
The diff has 380 additions and 34 deletions. Full-index SHA-256:
`7f5c282bf1e62d68e9b95a4182ae5fba2b8ba90c483720b833ba15dcb998b7ac`.
The whitespace check has no error. Sol reads all changed source, affected spawner/control/quiet contexts,
bounded test helpers, the real hold and tap forms, R-71, R-97, and the lead's dispositions.

### Source and separate clock scope

The control holds the worker's link at the accept edge, as R-71 requires.
The hold arms only for a Created session. The synchronous spawner connects the worker and marks its link before accept.
The worker exists and writes its hello normally. The edge records its returned identity without fabricating an identity.
Core accepts no held link or byte until release. It keeps Start incomplete and the session Starting.
No production machine, clock, pin, or mutation exclusion changes.

The accept selects the first unheld pending entry. Removing that entry preserves other unheld links' order.
Release removes the hold and signals the host wake. Releasing an armed hold disarms it.
The pending link retains the worker's bytes and byte order. Holds belong to each host's Faults map.
Equal instance identifiers in separate directories therefore share no hold.
Both controls validate the handle, session, and exact arguments.
Duplicate holds, absent releases, unknown handles/sessions, extra arguments, and holds after Start return Bad.
start_held is false while armed, true when the link is held, and false after release.
The existing payload-hold observation remains.

The quiet check excludes only held workers' link reports, as the real tap does.
It retains worker ready work, process exits, connection reports, and all other worker-link reports.
The new Core test observes quiet while the hello is held, then nonquiet immediately after release.
It observes Start completion and Running only after release.
Other tests cover first/middle held links, unheld accept order, retained byte order, disarm, control errors, and separate hosts.
The existing pump helpers have a finite 50-pass bound. The transport fixture drains its finite queued test bytes through nonblocking reads.

R-97 requires the worker's own startup deadlines to use a separate process-edge clock.
It excludes those deadlines from Core's next_deadline query and explicitly permits this control PR before the clock fix.
This PR leaves that defect unchanged, keeps a2_1_no_deadline_code_exists pending with an R-97 reason, and names P5 as owner.
The lead separately assigns the identity-hold capability and transcript repair. guardian_exit remains backlog work.
This verdict accepts no missing capability, clock workaround, or result from those future changes.

### Local diagnostic and exact-head evidence

Sol reads complete unpend.sh, real.sh, checks.sh, and their completed logs under shared/core-stage1/evidence/p5-hold.
The scripts set both parallelism caps to four. Their filtered pipelines supply named results, not each command's numeric exit.
unpend.log SHA-256: `003220ab6cbd09bb30a664853ef87767236274ec3263114d656a2edbecb675a7`.
It records a clean of 16366 files and 4.9 GiB, then names this worktree as the target writer.
Its source is base bfef8865 plus an uncommitted eight-file diff, 358 additions and 33 deletions.
It retains no complete patch hash. These results therefore remain diagnostic evidence for that working diff.
Strict seeds 0–31 pass attach-state. Other outcomes are startup step-7 FAIL, identity step-6 FAIL, and unsupported guardian_exit.
The script restores both pending files and Cargo.lock; the log reports those files clean.

real.log SHA-256: `3d4b1c4412258d162ee0f4a06507e8a90ce776f84cd5c9adf12d2652d31156cb`.
It names the same earlier working diff and prebuilds three candidates.
Real seed zero gives startup INCONCLUSIVE at step 23 and identity FAIL at step 6.
It restores the pending files and Cargo.lock. Its script states that this worktree remains the only target writer after the clean.
This is no exact-head execution or untouched-base failure proof.

checks.log SHA-256: `e2f8ca6adef931f9d369e0fa61c3ff03e2a04c6f86c4b3e9037f0992b80bea87`.
It names the exact clean head, rustc 1.97.0, macOS 27.0.1, and targets/p5.
It records fmt, filtered clippy completion, 305 host, 427 testkit, and 192 worker-core unit passes.
Strict seeds 0–31 report 259 conformance passes; taint passes; the final status is clean.
Its script states that the worktree remains the only target writer after the recorded clean.
That statement does not independently prove every earlier writer. The log supplies no local lists or public-api result.

### Completed gate and independent mutation audit

Gate: `gates/botster-core-stage1-p5-hold-start-403bbf68-pool-20261011-200046-33208.log` under `~/botster-sessions`.
SHA-256: `3e13590a828f0c5f79a10c20d28ae141b3c47837b3317f50c6adea34478ba8be`.
Sol reads the exact head/base header, list/coverage results, all raw mutations and citations,
relevant normal PASS records, conformance summaries, and final exits.
The job runs on MS-A1, kernel 6.12.111+deb13-amd64, with four CPUs.
All ten steps PASS in 437.0 seconds. Job and wrapper exit zero after 520/521 seconds, with zero queued seconds.
Default has 1867 passed and 432 skipped in 15.906 seconds.
Slow has 611 passed and 2046 skipped in 43.126 seconds.
Coverage checks 1076 slow-only tests and four named real proofs.

Sol independently parses 29 unique mutation identities: 21 caught, eight unviable, no miss or timeout, in 115.1 seconds.
All 21 raw catches equal all 21 named citations. Every catcher has a normal PASS record.
The first-link test catches the false hold predicate and changed equality.
The held-quiet test catches the empty held-worker list and changed control results.
Stage 2 does not run because stage 1 has no miss. No FLAKY token occurs.
The eight unviable outcomes supply no executed catch. Fuzz PASS reports no changed decoder harness.

All new hold tests PASS. Attach-state has testkit PASS in 0.411 seconds and real PASS in 0.178 seconds.
Both strict summaries report 259 passed and zero failures.
Held-full has 64 expected ignored results with zero failures in 19.28 seconds.
Sol verifies the sole pending removal is conf::a2_1_attach_state_rules; the count changes from 410 to 409.
Other pending changes are comments. All 33 reported real-pending passes stay in the unchanged 44-entry list.
The gate reports minimum testkit-passing 66/69, real-passing 59/68, and real-accepted 60/69.
The exact-head gate supports the removal; the earlier uncommitted diagnostic does not substitute for it.

### Body, package review, and disposition

Sol reads the complete corrected body, SHA-256
`5898072be82ebed8581f5450350157daf11b5d0e28a949bd26a230776dcbbafa`.
GitHub reports this exact head and base. The body distinguishes the actual parent from the PR base,
job from wrapper durations, all mutation outcomes, local taint from gate lists/API results,
and earlier uncommitted diagnostics from exact-head proof.
These corrections close package LOW R301-F1. Sol finds no additional source, body, or evidence condition.
Sol reads complete package Round 1 at `9f100371edc7b754d0ae6d466a32db909b36be96`.
Its CLEAN and retained limitations match this review.

Both initial reviews have zero open findings. The lead's final-slot import, applicable delta/checks,
exact-final-head gate, both final reviews, and Q7 remain required before merge.
This verdict covers only the reviewed source, corrected body, and completed first-READY gate.
Sol runs no build, test, gate, mutation, reversal, measurement, or executable base-check job. Never poll.

VERDICT: CLEAN (0 open integration findings; first-READY source, evidence, and body) at 403bbf689cc9f745a9552cc518374a6a8f2cdb32 against base bfef88653bb59ad8d3cd38da1e206f7aba64ce92
