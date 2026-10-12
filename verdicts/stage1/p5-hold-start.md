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

## Round 2 — train 1 member source delta

Train head: `be95ef13edc6d9eccf87a8c0b2c09b9cf820963a`.
Train tree: `456fdd44172ee85eb58ad443ec1e3dbf9f1dd2bc`.
Train base: `ae49bbec2d1086f918341bc596bf8560ba27bcd1`.
Frozen #301 member: `403bbf689cc9f745a9552cc518374a6a8f2cdb32`.
Frozen #304 member: `37e5ee32d326f588676b6fc727ad52c86022de61`.
Lead assignment: `msg_plugin-w_1791781317_4ac1d8`.

I read the complete steward-approved train rule and PR body in contracts BUILD PR #86 at `b6c7ea9d6abd37d723ecf491c618013eb36d9da7`.
That PR is open at this read; I do not claim it merged. The lead explicitly instructs this train under its approved rule.
The rule requires frozen member heads, ordered merge commits, member delta reviews for lists/shared helpers, and a combined integration review where required.
It also requires one exact-train-head gate, member final evidence receipts, equal base/gated trees, and a verified fast-forward push.
This round covers only #301's source composition. Sol-B owns #304 and the combined train review.

### Train identity and own patch preservation

I independently read both merge Git objects.
First merge `f3b80d034acf7872fa2cc4009002a2460a5d5329` has parents train base ae49bbec and frozen #301 head 403bbf68.
Its tree is `8ffdc4aaee69ecb159f569b9027b567c707b3375`.
The train head has parents that first merge and frozen #304 head 37e5ee32.
The member order matches the lead's instruction. The train whitespace check reports no error.
Combined full-index diff SHA256: `528c21f21fe4413e0cf1d2510ad3592b6b92855947a0aad3ae77f96da5adcd78`.

The first merge changes exactly #301's nine own paths. Every outside-own blob equals the train base.
For every own path, the accepted patch equals the new first-merge patch after removal of index and hunk-location metadata.
Accepted own diff SHA256: `7f5c282bf1e62d68e9b95a4182ae5fba2b8ba90c483720b833ba15dcb998b7ac`.
New first-merge diff SHA256: `4bc6ea8a302dd28de650dc678f40968f88b3b5b8759e584460ab99a8978432cf`.
Seven source blobs are byte-identical to the accepted member.
The worker blob retains the same #301 patch plus v1's imported six-line `add_node` helper.
The final #304 merge leaves all eight #301 source blobs equal to the first merge.
It overlaps #301 only in core-pending.txt.

### Imported worker helper and interface composition

I read the complete worker delta from #301's original base bfef8865 to the train base.
It contains only `add_node`, compiled for unit tests and private to the crate.
The helper adds a node to the existing worker simulation for the already accepted route-progress test from #299.
It changes no production worker path, clock, host pump, process identity, hold map, accept order, or report filter.
The train retains that helper and #301's `holds_reports_but` and `edges_quiet_but` changes.
The quiet control still reads the applicable host's held workers and process table.
It excludes only held worker link reports. Ready worker work, exits, other connections, and unheld worker reports still count.
The hold remains at the injected accept edge. Release removes the hold and signals the host wake without replacing the buffered link.
All hold controls, per-host state, accepted byte order, and tests retain the reviewed source.

The #304 merge adds its reviewed proof mapping and real-process test changes.
It changes no testkit hold control or report filter. It supplies no acceptance evidence for #301's own removal.
The combined source review separately checks the member interaction.

### Shared conformance accounting

I independently check all nine conformance text sets after each merge, using each member's merge base.
Both three-way formulas PASS for every set. Every base removal remains removed.
Pending counts: original #301 base 410, accepted member 409, train base 409, first merge 408, final train 407.
The final train removes exactly these ids from its base:

- #301: `conf::a2_1_attach_state_rules`.
- #304: `conf::dp_2_stream_handoff_transfers_ownership_and_closes_host_copy`.

No pending id is added. The remaining #301 explanatory comments and pending ids remain.
The train real-pending set has 43 entries, held has four, ledger has 691, and minimum has 69.
The #304 real-proof additions are its three accepted entries; they do not change #301's harness proof.
The removed attach-state transcript is byte-identical at .41 and the train's .43 pin.
Its SHA256 is `57f313c9d98cd2ad6ce5e4d65dff0426dc425d251225604cb5d3bda99c873990`.
The train changes no imported pin, dependency profile, CI file, build-root rule, or mutation exclusion.

### Required source receipts and limits

I read complete package #301 train Round 2 at `78a42cd654105b23aa7362cd81689cd54679242b`.
Package receipt: `msg_plugin-w_1791781752_b81ef9`.
Its exact identities, source composition, imported helper, pending removals, and retained limits agree with this review.
I also read Sol-B's complete combined source review and #304 train delta at `3deaf0a165eb877a4a6fc29b8afe87b6ff2daeab`.
Sol-B receipt: `msg_plugin-w_1791781706_52d460`.
That combined review has zero interaction findings and retains #304's separate composed-proof and provenance limits.

No #301 source-delta finding remains. The initial source/evidence verdict remains `500639cb61565a1a42adadf7e46938728fdb6d81`.
R-97's separate clock fix, identity hold, and guardian-exit work remain outside this member review.
The lead and steward permitted #301 before the separate R-97 fix. The train adds no workaround for it.
The original source, body, local provenance, platform, and first-gate limits remain.
This round accepts no train gate result and does not replace the required member final evidence receipts.
The completed exact-train-head gate, all final receipts, base-tree verification, and verified push remain due under the train rule.
I ran no build, test, gate, mutation, reversal, measurement, or executable base check.

VERDICT: CLEAN (0 open #301 source-delta findings; train evidence pending) at be95ef13edc6d9eccf87a8c0b2c09b9cf820963a against base ae49bbec2d1086f918341bc596bf8560ba27bcd1
