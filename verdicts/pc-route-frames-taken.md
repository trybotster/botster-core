# PC package review: route_frames_taken (#319)

## Round 1: VERDICT: NOT CLEAN (2 open)

Reviewed head: `ab9b223611978270179cb2c2f361f3a2becfbe65`.
Source merge base: `080987829a2e2245a7ea97606020400392c974d6`.
Tree: `d926f7aea682779594933d840ad9c5a8c50f8a03`.
READY: `msg_plugin-w_1791827733_63e796`.
Lead role ruling: `msg_plugin-w_1791827904_c61baa`.
The lead confirms 0179 as package reviewer and Sol as integration reviewer. This replaces PC's reversed role labels.
Risk tier: HIGH, BUILD rule 3, because this PR changes the shared testkit.
Binding plan: revision 23cm, `~/botster-sessions/pins/stage1-plan.c77c2434.md`.

### CQ-RFT-1: MEDIUM, open — new byte decoders have no required fuzz harness

Locations: `crates/botster-core-testkit/src/route_client.rs:84`, `crates/botster-core-testkit/src/real/rqueue.rs:122`, and `xtask/src/ci.rs:69`.

`FrameEnds::push` decodes client frame-length prefixes and tracks complete frame boundaries.
`diag::parse` decodes the new binary `unix_diag` reply, including its message header and attributes.
The PR adds no bolero harness for either byte decoder.
The landing harness list contains no testkit entry. It remains unchanged in this PR.
The exact-head gate reports `fuzz: the diff changes no crate with a decoder harness`.
Its fuzz step takes 0.0 seconds. No decoder fuzz run occurs.

Plan section 8, step 9 requires a bolero harness for every byte decoder that a package adds.
The existing harness-list comment states the same requirement.
The named unit tests and mutation catches do not replace this required coverage.
The gate's PASS result therefore does not supply all required decoder evidence for this HIGH PR.

Required change:

1. Add bounded bolero coverage for the new byte decoder paths.
2. Cover arbitrary and incomplete byte input, split writes, and complete-frame counts for the frame tracker.
3. Cover malformed, truncated, and valid binary replies for the diag decoder.
4. Register the harnesses in the landing selection.
5. Make the diag harness compile under the selected fuzz command despite the current `slow` module gate.
6. Supply actual fuzz records on the corrected review head. Retain any discovered crash input as a regression.

Package notice to PC: `msg_plugin-w_1791828105_9d39a7`.
Package notice to Sol: `msg_plugin-w_1791828105_c45109`.
No reviewer execution job runs to prove this missing-harness finding; source and the completed log establish it.

### CQ-RFT-2 / R319-SI-1: MEDIUM, open — an invalid errno can panic instead of returning a harness error

Location: `crates/botster-core-testkit/src/real/rqueue.rs:135`.
Sol source receipt: `msg_plugin-w_1791828457_7b7b67`.
The reviewer independently confirms the decoded-value path in the reviewed source.

An `NLMSG_ERROR` reply reads an arbitrary signed 32-bit errno at byte offset 16.
The parser then calls `io::Error::from_raw_os_error(-errno)` without checking the negation.
A 20-byte reply with declared length 20, message type 2, and `i32::MIN` at offset 16 reaches this expression.
Negating `i32::MIN` overflows when overflow checks are enabled. The parser panics instead of returning `Err`.
R-93 condition (b) requires a harness error when the platform reply cannot be parsed.
The pure decoder must reject this invalid error value before conversion.

Required change: use checked negation or validate the errno range before conversion.
Retain the boundary reply as a regression in the new diag harness.
This finding uses static source evidence. No reviewer reproduction runs.

### Source review

The reviewer reads the binding PC brief first, the common instructions, BUILD.md, the plan, and the R-93 rulings.
The reviewer reads the complete seven-file diff, serving-point design, stream accounting, attach path, and worker receive/apply path.
The source counts client frames at the stream boundary. It adds no worker queue counter or production test branch.
The testkit reads the existing `taken[side]` counter for client writes that the worker end has read.
The real tier counts bytes accepted by `flush`, excluding bytes that still wait in the client.
It subtracts the worker end's unread bytes with `checked_sub`; an impossible negative count is a harness error.
The frame tracker processes one byte per loop step. It counts a frame only when its last byte is taken.
Split prefixes, incomplete bodies, and an empty body have explicit tests.

The tapped attach path reads the worker socket identity before Core takes the endpoint.
The route retains its client endpoint and caches that endpoint's socket identity.
The Linux query requests the worker inode's peer and receive queue, then checks the peer against the client inode.
It takes one nonblocking reply. Call, missing-socket, and peer failures return errors.
The invalid errno path remains CQ-RFT-2.
The macOS query uses absolute `/usr/sbin/netstat`, `LC_ALL=C`, and the existing cleanup deadline.
Its parser requires the named header columns and exactly one worker-address/client-connection row.
Tool failure, timeout, invalid output, no row, and duplicate rows return harness errors.
Neither query duplicates the worker endpoint or reads worker memory.

R-93's refusal-room premise remains explicit in the worker design paragraph and PR body.
The separate contracts PR must add the driver dispatch, controls row, premise checks, and transcript fences before reliance on the control.
The per-seed proof uses a scratch contracts tree; #319 does not move the contracts pin.
No conformance list, facade snapshot, manifest, lockfile, mutation exclusion, or gate-decision file changes in this head.
The two AM-2 ids remain pending. This review authorizes no removal.
The source merges base `08098782` once in `a93d7d1e`, without conflicts.
The body's Prior art note identifies the old reader-side `FIONREAD` code and rejects its unsafe access path.
No other source finding is identified after CQ-RFT-2.

### Completed gate evidence

Log: `~/botster-sessions/gates/botster-core-stage1-pc-route-frames-taken-ab9b2236-pool-20261012-103338-83559.log`.
Verified SHA-256: `87d4fc09cb7b8e84f93b384ce087092b684014b48e045fe5ca8b8cffe43b2512`.
The retained `gate-client-ab9b2236.out` is byte-identical to the canonical log.
The log names the exact head, MS-A1, four CPUs, and base `9fe97f506a1bb5cef0d935b2ba2ccf3ff9ba90ec`.
It reports source merge base `08098782`; this is a pre-review gate, not a current-base merge receipt.
The prebuild writes the candidate manifest for three binaries.

All ten checks report PASS, with a total of 1263.5 seconds.
The job exits 0 after 1273 seconds. The wrapper exits 0 after 1274 seconds.
Default tests: 1937 passed, 431 skipped, 11.848 seconds.
Normal slow tests: 627 passed, 2070 skipped, 40.189 seconds.
Unmutated Stage 2 baseline: 627 passed, including ten slow tests, 2830 skipped, 39.775 seconds.
Both strict summaries report 261 passed, zero failed, and 430 ignored.

Stage 1 has 86 mutants: 15 caught, 71 missed, zero timeout, and zero unviable.
Stage 2 has 71 mutants: 67 caught, four unviable, zero missed, and zero timeout.
Total: 82 caught, four unviable, zero missed, and zero timeout.
Every raw catch matches a named citation. All 82 citations have normal PASS records.
Every Stage 2 citation also has an unmutated baseline PASS. The log has no FLAKY or RETRY record.
The four unviable replacements supply no behavioral catch.
They affect a negative compile-time request length and three unavailable `Default` implementations.
The Linux peer-check mutant is caught by `bytes_that_wait_in_the_client_are_not_taken`.
The gate runs the real socket/count/error tests and both platform parser test groups.
The fuzz check's empty selection remains CQ-RFT-1; this review does not count it as executed fuzz evidence.

### Other supplied evidence and body

The Linux probe runs on MS-A1 with uid 1000. Its exact lookup and dump each report unread bytes 0, 100, 70, and 0.
The peer socket agrees with the client's inode. The worker's own `SIOCINQ` agrees after the reads.
The probe reports NOT FOUND after worker exit and exits 0.
The Mac probe reports the same unread values in the paired `Address` and `Conn` rows on macOS 27.0.1.
These records establish the ruled access path; they are not an execution of the final Core head.

The current-head local record identifies `ab9b2236` and a clean marker at `2026-10-12T17:25:52Z`.
It reports ten xtask exit codes 0, both clippy exit codes 0, testkit 442/442, focused slow 13/13, and strict 261/0.
The script filters test output and does not retain numeric test exits or the clean exit.
These are local summaries with those limits. The exact Linux gate independently supplies its test results.

The red-on-revert record identifies Core `22888a13` and a clean marker before its passing baseline.
All 13 applicable Mac changes produce a failed test with exit 100. The Linux-only peer change is not compiled there.
The record restores the source with a clean status; it does not run a new final restored suite.
The exact Linux gate supplies the peer-check mutation catch omitted on the Mac.
The focused Linux mutation record on `70679e8a` reports 30 tested, 28 caught, two unviable, and exit 0.
The body retains all three earlier RED gate results and identifies their fixes without relabeling them as GREEN.

The per-seed record identifies Core `22888a13` and scratch contracts `1baa18ed` before a temporary path override.
Its script sets `BOTSTER_SEEDS=0-31` and records testkit and real exits 0 for fenced `am_2_one`.
The real run takes 8.41 seconds. The script restores the manifest and lockfile with clean status.
The clean marker has no numeric clean exit. This is earlier-head, modified-contracts evidence, not the current pinned transcript or final gate.

The reviewer reads the complete READY body and its later role correction.
Current body SHA-256 without an added line feed: `a69a136905ffaf4c5b38623cc61266ed1e0bcc916724e57a5281986a03928e72`.
The correction adds only the lead's reviewer roles and a trailing line feed.
The body labels the source merge base and gate base separately and preserves the evidence scopes above.

### Next step

CQ-RFT-1 and CQ-RFT-2 must close before package CLEAN. Sol's complete independent integration review remains due.
Any changed head needs its delta review and required evidence.
The current-base final slot, base check, final gate, final receipts, and lead Q7 remain due after both CLEANs.
No reviewer build, test, gate, mutation, reversal, reproduction, or executable base-check job runs.
Wait for the doorbell. Never poll.

### Matching complete integration verdict

Sol receipt `msg_plugin-w_1791828711_014353` supplies commit `a03e76a98afd1e049f3be9706b85a65cba6b19a7`.
The reviewer reads its complete Round 1 in `verdicts/stage1/pc-route-frames-taken.md`.
The remote integration branch publishes that exact commit.
The verdict matches package `1428339dae1b7b7998bd61585b2cfaf0556a9d39` on head `ab9b2236`, source base `08098782`, and tree `d926f7ae`.
Both complete reviews have CQ-RFT-1 and CQ-RFT-2 / R319-SI-1 open, each MEDIUM.
Both preserve the initial gate, empty fuzz selection, and earlier evidence scopes.
No additional integration finding occurs. No correction is accepted before its completed READY and delta reviews.
Wait for the correction doorbell. Never poll. No reviewer execution job runs.

## Round 2: VERDICT: CLEAN (0 open; repair and text delta)

Reviewed head: `fabbf66cdaccedd942b8cedea824c763bd7bfa7a`.
Source merge base: `080987829a2e2245a7ea97606020400392c974d6`.
Tree: `56a6056cbe91f05850e81ab724db39aed3b5992f`.
Gated parent: `d1aa733480bbc9ee5449e70c6b34939fa18065e9`, tree `6b70e352c35172291a483c8445233ff96d2bc4fd`.
Repair READY: `msg_plugin-w_1791836503_dcec8b`.
Final text receipt: `msg_plugin-w_1791836833_3ea519`.
Scope: the complete repair from `ab9b2236`, its completed evidence, and the final R-115 text correction.
The reviewer reads the PC brief first. BUILD.md, plan 23cm, and the applicable steward rulings remain binding.

### CQ-RFT-1: closed

Commit `e8cb21ef5d785e766742222d0606f323e2584d86` adds both bolero harnesses and registers them for landing.
The testkit uses the existing workspace bolero version as a dev dependency. The lockfile adds only that dependency edge.
The pure diag decoder moves to unconditional `sock_diag.rs`, so its harness builds without `slow` on both operating systems.
`FrameEnds` exposes its boundary and count methods within the Rust-only testkit; the Core facade does not change.
No OS call moves into the decoder. No new production test branch appears.
The xtask delta adds two entries to the existing harness list. It adds no new check family under R-115.
The existing harness-presence and changed-crate selection tests pass in the completed gate.

`frame_ends_decoder` compares chunked and whole-input tracking against a separate whole-input reference.
It checks complete-frame boundaries and the count at each last byte and one byte before it.
It also constructs up to 64 valid frames with small bodies, so coverage does not depend on random bytes forming a valid prefix.
`sock_diag_decoder` checks arbitrary replies, structured valid replies, both attribute orders, wrong inodes, and truncated replies.
It includes the invalid errno boundary on every property case.
Each property test has 256 cases. The default tier supplies the pinned bolero seed through the existing tier environment.
Both property tests have normal default PASS records.
Both landing fuzz harnesses actually run in the completed gate and finish without a reported crash.

### CQ-RFT-2 / R319-SI-1: closed

Commit `86fdb2096e075f94a71d3d6b014710d685ffd1b9` changes errno conversion to `checked_neg()` followed by a positive-value check.
`i32::MIN`, zero, and a non-negative source errno return an error without unchecked negation.
The unit regression covers `i32::MIN`, zero, and valid `-1` with its expected OS error.
The property harness retains the 20-byte `NLMSG_ERROR` boundary and checks that it returns an error.
The unit regression passes in the normal slow tier and unmutated baseline.
The property harness passes in the default tier and completes its fuzz run.
The retained Mac reversal removes the checked conversion and fails both the property harness and the unit regression.
Its numeric nextest exits are 100. A separate reversal admits zero and fails the unit regression with exit 100.

### Earlier repair gate and surviving-mutant closure

The completed gate on `86fdb209` remains RED: `FrameEnds::ends` replaced by a leaked empty vector survives both stages.
Stage 1 reports 45 caught, 44 missed, and three unviable. Stage 2 reports 42 caught, one missed, and one unviable.
Fuzz does not run in that RED gate. The job exits 1 after 1220 seconds; the wrapper exits 1 after 1221 seconds.
Commit `d1aa7334` changes the unit assertion to read `ends()` and adds valid generated frames to the property harness.
The completed new gate catches the former survivor in Stage 1 through the frame-boundary unit test.
The separate `ends-mutant-d1aa7334.log` has two failed-test records, but no head, clean, diff, or numeric-exit provenance.
This review treats that short record as supporting output. The exact gate supplies the accepted mutation proof.

### CQ-RFT-3: LOW, closed — FLAKY and pending-status labels

Notice to PC: `msg_plugin-w_1791836665_0d8342`. Correction receipt: `msg_plugin-w_1791836711_3763b3`.
The first repair body omits two Stage 2 FLAKY outcomes for `conf::am_2_one_admission_point_orders_all_sources`.
They occur under `sock_diag.rs:64:5` `i32_at -> Some(1)` and `sock_diag.rs:85:32` `>` replaced by `<`.
Each trial fails, then passes on its permitted retry. Neither trial earns catch credit.
Each mutant is caught instead by `real::rqueue::slow_tests::the_diag_reply_gives_the_peer_and_the_unread_bytes`.
The raw outcomes, citations, normal PASS, and baseline PASS records establish those separate valid catches.

The corrected body records both FLAKY outcomes and the independent catching test.
PC attributes the flake to the existing unfenced cross-source order described by R-93.
The structural fix has owner PC on contracts branch `stage1/pc-route-frames-taken`, reported head `bdad2be3`.
PC reports that its gate is running and its PR is not yet open. This reviewer reads no running output.
The contracts fence and its later Core pin remain separate prerequisites; #319 must serve the control before that pin.
This receipt accepts neither the future contracts head nor a completed pin fix.

The earlier body and this review's Round 1 incorrectly say that both AM-2 ids are pending.
The reviewer independently checks both exact-head pending files; the repair changes neither file.
`conf::am_2_one_admission_point_orders_all_sources` is absent from both and runs as a required test on both tiers.
`conf::am_2_fairness_bound_across_routes` remains in `core-pending.txt`.
The corrected body states that distinction. This correction supersedes the earlier review's pending-status statement.
No pending-list addition or removal occurs or is approved here.

### CQ-RFT-4 / R319-SI-2: LOW, closed — R-115 reviewer premise

Sol source notice: `msg_plugin-w_1791836722_79edd5`.
Package body/design notices: `msg_plugin-w_1791836790_5edb10` and `msg_plugin-w_1791836790_8598a9`.
R-115 stops the proposed R-93 premise-check extension. The premise becomes reviewer checklist item 6.
The earlier body and worker design paragraph still claim that the order lint checks it.
Commit `fabbf66c` changes only three lines of `crates/botster-worker-core/DESIGN.md`.
The reviewer reads the complete text delta and the corrected body.
Both require no route gate or accept limit from the first frame to the fence, with `route_queue_bytes` at the Core default.
Both identify the reviewer checklist under R-115. No new lint or check is added.
The existing R-93 order lint remains separate and unchanged.
The prior review's requirement for a new premise check is superseded by R-115.

### Completed parent gate

Log: `~/botster-sessions/gates/botster-core-stage1-pc-route-frames-taken-d1aa7334-pool-20261012-125446-96535.log`.
Verified SHA-256: `c42efaa14229841b83d677f915c591ef7f2042c2e8d01f7491a87259cfcb0462`.
The retained `gate-client-d1aa7334.out` is byte-identical to this canonical log.
The gate names parent `d1aa7334`, MS-A1, four CPUs, and base `2683de7ccbb1360d124422b3b6a2dd196b046c7e`.
Its source merge base remains `08098782`. This is repair evidence before the final current-base import.
The candidate prebuild writes the manifest for three binaries.

All ten checks PASS. The CI total is 1474.5 seconds.
The job and wrapper each exit 0 after 1571 seconds; the job queues 88 seconds and runs 1483 seconds.
Default tests: 1939 passed, 431 skipped, 11.954 seconds.
Normal slow tests: 627 passed, 2072 skipped, 40.476 seconds.
Unmutated Stage 2 baseline: 627 passed, including ten slow tests, 2832 skipped, 40.977 seconds.
The complete baseline step takes 51.2 seconds. Both strict summaries report 261 passed, zero failed, and 430 ignored.

Stage 1 has 92 mutants: 46 caught, 43 missed, three unviable, and zero timeout.
Stage 2 tests exactly those 43 misses: 42 caught, one unviable, zero missed, and zero timeout.
Total: 88 valid catches, four unviable, zero missed, and zero timeout.
All 88 raw catch identities are unique and match their citations exactly. Every citation has a normal PASS record.
Every Stage 2 citation also has a baseline PASS record. All 43 miss identities match the 42 catches and one unviable outcome.
The four unviable identities match the earlier unviable replacements after the module move.
The two reported FLAKY outcomes remain explicit and earn no credit, as described above.

Fuzz selects both new testkit harnesses for 60 seconds each on the pinned nightly.
`frame_ends_decoder` completes 3195602 runs in 61 seconds.
`sock_diag_decoder` completes 5847439 runs in 61 seconds.
The fuzz step takes 325.6 seconds including its builds. Neither harness reports a crash.

### Supporting records, dependency, and final body

The current parent-head Mac record names `d1aa7334` and a clean marker at `2026-10-12T19:21:00Z`.
It reports ten xtask exits 0, both clippy exits 0, testkit 444/444, focused slow 13/13, and strict 261/0.
The existing filtered script does not retain numeric test exits or the clean exit. Those limits remain.
The exact Linux gate independently supplies the property and real-test results above.

The `86fdb209` reversal record has a clean status before its recorded clean output and passing baseline.
The baseline passes four default and twelve slow tests with numeric exits 0.
All 15 applicable Mac changes fail a named test with exit 100. The Linux-only peer change does not compile there.
The source restore has a clean status and names `86fdb209`; no new restored suite runs.
The current gate independently covers the unchanged Linux peer check.
The earlier probe, scratch-contracts per-seed proof, and RED gate records keep their original heads, platforms, and limits.

The reviewer reads R-111 and independently verifies contracts #100 MERGED at `49ff52d218ea55afae1fc120ddfb928a908bac04`.
Its existing kernel-state list contains `pty_queued`. The separate route-fence change must add `route_frames_taken` before reliance on its wait.
R-111's driver re-check does not add a wait to this Core control. #319 adds no transcript or pin.
The KSTATE-1 body note separates the merged driver fix from the later route fence and Core pin.

The reviewer reads the complete final body on `fabbf66c` and its complete corrections.
Body SHA-256 without an added line feed: `a0c173944b0ea479572145c236f38c3c1a0aaaf3426b3838d5f77ecaeaa9175e`.
Body SHA-256 with one added line feed: `a34789079ab414b91f7d174c33e7e36536d14bfcb18677b22ffa56c04b720c29`.
These labels preserve the parent gate, RED repair history, FLAKY outcomes, and local/scratch evidence limits.

### CLEAN scope and next step

All four package findings are closed. No new source, body, or supplied-record finding remains open.
The only change from gated parent `d1aa7334` to reviewed head `fabbf66c` is the inspected design text.
All code, tests, dependencies, conformance files, facade, and gate files are byte-identical between those heads.
The parent gate supplies unchanged-code evidence; it is not an execution on `fabbf66c`.
No exact-head gate on `fabbf66c` or final current-base import is claimed.
The matching complete formal integration repair verdict remains due.
After both CLEANs, PC must supply the required current-base compile check before FINAL-READY.
The lead's final slot, current-base import/base check, applicable delta reviews, exact final gate, final receipts, and Q7 remain due.
The contracts fence, kernel-state list addition, and later Core pin remain separate work with owner PC.
No later head, new status removal, specific retry, or completed merge is accepted here.
No reviewer build, test, gate, mutation, reversal, reproduction, or executable base-check job runs.
Wait for the doorbell. Never poll.
