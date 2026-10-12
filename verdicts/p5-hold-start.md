# P5 review — #301, testkit hold_start

## Round 1 — 2026-10-11

- PR: <https://github.com/trybotster/botster-core/pull/301>
- Head: `403bbf689cc9f745a9552cc518374a6a8f2cdb32`
- Tree: `2cc7ab25a4fd770876ae1d0e6e49fb9e9a0abcd5`
- Base: `bfef88653bb59ad8d3cd38da1e206f7aba64ce92`
- Parent: `ae3fbfff24b9b5ced6aa7a53bbd06c394c603ef2`, whose parent is the base above.
- READY: `msg_plugin-w_1791774608_1e2cbe`.
- Risk: HIGH, rule 3, the shared testkit crate.

I read the complete nine-file diff, affected control and edge contexts, R-71, R-97, and the complete corrected PR body.
The diff has 380 additions and 34 deletions. Its full-index SHA256 is `7f5c282bf1e62d68e9b95a4182ae5fba2b8ba90c483720b833ba15dcb998b7ac`.
The merge base equals the named base. The diff check reports no whitespace error.
The change moves no pin, production machine, startup clock, or mutation exclusion.
The sole pending-id removal is `conf::a2_1_attach_state_rules`. Other pending changes are explanatory comments.

### Source review

The hold follows R-71's accept-time form. Each host owns its `Faults` and pending links.
The control arms a hold for a Created session. The spawner connects the worker and marks that link before the host can accept it.
After the spawn succeeds, the edge records the worker identity for the quiet check.
The worker exists and writes its hello normally. The host receives no accepted link until release.
The edge adds no hook to a production machine and fabricates no process identity.

`accept_link` selects the first unheld pending link. Removing that entry preserves the order of all other unheld links.
Release removes the hold and signals the host wake. A release before connection disarms the hold.
The existing pending link retains the worker's bytes; release changes no byte or byte order.
Per-host hold maps prevent equal instance ids in separate directories from sharing a hold.

Both controls parse their arguments and validate the current handle and session row.
A duplicate hold, missing release, unknown handle or session, unsupported argument, or hold after Start is Bad.
`start_held` is false while armed, true after the link is held, and false after release.
It also preserves the existing payload-hold observation.

The quiet check omits the held worker's control-link report, as the real tap does.
It retains worker ready-work, process exits, other connections, and other worker-link reports.
The new Core scenario proves quiet while the hello is held and nonquiet immediately after release, before the host reads it.
It also proves that Start stays incomplete and Starting while held, then completes Running after release.
Other tests cover first and middle held links, unheld accept order, buffered byte order, release, disarm, errors, and separate hosts.
The control scenarios use the existing bounded pump helpers. The transport fixture drains only its finite queued test bytes through nonblocking reads.

### Startup-clock and pending scope

R-97 resolves the earlier clock question. The worker's own startup deadlines must use a separate process-edge clock, and `next_deadline` must exclude them.
R-97 explicitly permits this hold-control PR to proceed before that separate fix.
This PR leaves the existing clock behavior unchanged. It retains `a2_1_no_deadline_code_exists` with an R-97 reason and P5 owner.
It also retains the missing identity-hold and guardian-exit cases. This verdict accepts neither missing capability nor a clock workaround.

### R301-F1 — LOW — body evidence precision, closed

I requested four body corrections in `msg_plugin-w_1791774776_b38312` and informed Sol in `msg_plugin-w_1791774776_88c968`.
The body confused the PR base with the head's parent, gave the job duration as the wrapper duration, omitted mutation outcomes, and overstated the local checks log.
It also needed to identify the earlier uncommitted diagnostic runs separately from exact-head proof.
P5 correction receipt `msg_plugin-w_1791774844_244d4f` names the unchanged reviewed head.
I read the complete corrected body. It now gives the exact parent and base, job 520 seconds and wrapper 521 seconds, and all mutation counts.
It attributes lists and public-api success to the gate. The local log claim covers taint only.
It explicitly scopes `unpend.log` and `real.log` to the earlier working diff. R301-F1 closes before publication of this round.

### Local evidence

I read the complete `unpend.sh`, `real.sh`, and `checks.sh` and their completed logs under `evidence/p5-hold`.
`unpend.log` SHA256: `003220ab6cbd09bb30a664853ef87767236274ec3263114d656a2edbecb675a7`.
It records a clean of 16366 files and 4.9 GiB, then names this Core worktree as the target writer.
It runs on base `bfef8865` with an uncommitted eight-file diff, 358 additions and 33 deletions.
P5 identifies that source as the first commit's working diff. The log retains no complete patch hash, so I accept it as diagnostic evidence.
Its strict seed 0–31 trial passes attach-state. The other raw outcomes are startup step 7 FAIL, identity step 6 FAIL, and unsupported `guardian_exit`.
The script restores pending files and Cargo.lock. The log reports those files clean.

`real.log` SHA256: `3d4b1c4412258d162ee0f4a06507e8a90ce776f84cd5c9adf12d2652d31156cb`.
It names the same earlier working diff and prebuilds three candidate binaries.
At real seed 0, startup is INCONCLUSIVE at step 23 and identity fails at step 6. The script restores the pending files and Cargo.lock.
Its script states that no other worktree wrote to the target after the preceding clean.
These are diagnostic outcomes on the earlier working diff, not current-head execution claims.

`checks.log` SHA256: `e2f8ca6adef931f9d369e0fa61c3ff03e2a04c6f86c4b3e9037f0992b80bea87`.
It names this exact clean head, rustc 1.97.0, and macOS 27.0.1.
Its script states that this worktree remained the target's sole writer after the earlier clean.
The filtered output records fmt and clippy success, 305 host, 427 testkit, and 192 worker-core unit passes.
It records 259 conformance passes with strict seeds 0–31, taint PASS, and a clean final status.
It retains no local lists or public-api result. Those results come from the completed gate below.
The filtered pipelines supply named outcomes and summaries, not numeric exits for each command.

### Completed exact-head gate

Log: `~/botster-sessions/gates/botster-core-stage1-p5-hold-start-403bbf68-pool-20261011-200046-33208.log`.
SHA256: `3e13590a828f0c5f79a10c20d28ae141b3c47837b3317f50c6adea34478ba8be`.
The header names this exact head and base. The lists step confirms the same merge base.
All ten steps PASS in 437.0 seconds. The job exits 0 after 520 seconds with no queue wait; the wrapper exits 0 after 521 seconds on msa1.
Default: 1867 passes and 432 skips in 15.906 seconds. Slow: 611 passes and 2046 skips in 43.126 seconds.
Coverage checks 1076 slow-only tests and four named real proofs.
All new hold tests pass. Attach-state passes on the testkit in 0.411 seconds and on the real tier in 0.178 seconds.

Mutation has 29 unique outcomes: 21 caught, 8 unviable, no missed mutant, and no timeout, in 115.1 seconds.
No stage 2 is needed. All 21 raw catches match the 21 citations, and each cited test has a normal PASS.
The held-first test catches the hold predicate's false and comparison mutants. The held-quiet scenario catches the empty held-worker list and changed control results.
The complete log has no FLAKY outcome. Fuzz reports no changed crate with a decoder harness; no new fuzz execution is claimed.

Both strict summaries report 259 passes and no failure. The pending check runs 321 of 339 ids, with 18 real-only ids excluded and 3 held.
The held budget check reports 64 ignored trials and no failure in 19.28 seconds.
All 33 reported passing ignored real ids remain in the unchanged 44-entry real-pending list.
Minimum counts are testkit-passing 66/69, real-passing 59/68, and real-accepted 60/69.
The exact-head passing attach-state trials and strict checks support the sole id removal.

### Disposition

Zero package source, body, or evidence findings remain on this head.
This CLEAN covers the complete source, corrected body, and completed first-READY gate.
The matching integration verdict and the lead's final-slot and Q7 rules remain required before merge.
A later source change or base import needs its applicable review and exact-head evidence.
R-97, the identity hold, and guardian-exit remain separate work.
I changed no product code and ran no build, test, gate, mutation, measurement, or executable merge check.

VERDICT: CLEAN
