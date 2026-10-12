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

## Round 2: VERDICT: NOT CLEAN (1 open; repair and text delta)

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

### Review scope and next step

The four earlier package findings are closed. CQ-RFT-5 below remains open.
The only change from gated parent `d1aa7334` to reviewed head `fabbf66c` is the inspected design text.
All code, tests, dependencies, conformance files, facade, and gate files are byte-identical between those heads.
The parent gate supplies unchanged-code evidence; it is not an execution on `fabbf66c`.
No exact-head gate on `fabbf66c` or final current-base import is claimed.
CQ-RFT-5 must close before package CLEAN. The matching complete formal integration repair verdict remains due.
After both CLEANs, PC must supply the required current-base compile check before FINAL-READY.
The lead's final slot, current-base import/base check, applicable delta reviews, exact final gate, final receipts, and Q7 remain due.
The contracts fence, kernel-state list addition, and later Core pin remain separate work with owner PC.
No later head, new status removal, specific retry, or completed merge is accepted here.
No reviewer build, test, gate, mutation, reversal, reproduction, or executable base-check job runs.
Wait for the doorbell. Never poll.

### CQ-RFT-5 / R319-SI-3: MEDIUM, open — new property binary lacks the required build-root test

Location: `crates/botster-core-testkit/tests/props.rs`.
Sol source notice: `msg_plugin-w_1791837054_078357`.
The reviewer reads current base `3e655eff46d36f8c337339a1a17d401c8c662594`, `xtask/src/slow_coverage.rs:195` and its callers.
Its existing `presence` check requires the build-root test in every listed default and slow binary.
The test must run and must not be ignored. The check explicitly requires `botster_test_process::built_in_test!();` for a libtest binary.
The new `tests/props.rs` binary has both decoder tests but no root macro.
Its old source base predates this coverage check. The READY gate therefore cannot detect this current requirement's violation.
The missing test leaves the new binary outside the required B13 build-root verification.

Required change: add the existing `botster_test_process::built_in_test!();` macro to `tests/props.rs`.
Retain both decoder harnesses. Supply the corrected exact head and the applicable marker/coverage evidence.
This is an existing coverage requirement, not a new check or an R-115 freeze exception.
The reviewer proves the omission from source and runs no test or reproduction.

Package commit `cf37cdc788fbaeb669d4f2d0b7d0aed2d478df5c` published CLEAN before the reviewer read Sol's new finding.
That premature CLEAN is superseded. No terminal package CLEAN from that commit was sent to the lead.
Round 2 remains NOT CLEAN with CQ-RFT-5 open. The four earlier closures and their evidence scopes remain valid.

### Matching complete integration repair verdict and import decision

Sol receipt `msg_plugin-w_1791837647_fb7734` supplies commit `7cc7f0ac08b5b4788de403834a84bdcf4d0bac71`.
The reviewer reads its complete Round 2 in `verdicts/stage1/pc-route-frames-taken.md`.
The reviewer verifies that the remote integration branch publishes that exact commit.
It matches package `80d96b69804fda10d7f13eaa26f8bee4aa6ea462` on head `fabbf66c`, source base `08098782`, and tree `56a6056c`.
Both complete reviews close the four earlier findings and retain CQ-RFT-5 / R319-SI-3 as MEDIUM.
Both preserve the parent gate and its two FLAKY outcomes without catch credit.

The reviewed head has no `macro_rules! built_in_test` in `botster-test-process`.
Current base `3e655eff` defines that macro in `src/built_in.rs:127`.
The marker fix therefore needs the newer dependency API before it can compile.
Sol has asked the lead to decide whether PC can import that base before the final slot.
This receipt records that pending decision; it approves no early import or provisional CLEAN.
Wait for the lead decision or completed correction doorbell. Never poll. No reviewer execution job runs.

Sol relay `msg_plugin-w_1791837724_855543` supplies lead decision `msg_plugin-w_1791837657_ac3742`.
The lead permits PC to import current `v1` once and add the existing root macro in the same step, before FINAL-READY.
The relay names current landed base `007efbde6a59af403d0937c38361c137a8961a48`.
The lead states that the train model has no final slot. This replaces the earlier slot-timing condition for this work.
Both reviewers must inspect the resulting exact delta. The corrected head and applicable evidence remain due.
CQ-RFT-5 stays open. No new head or execution result is accepted by this decision receipt.

## Round 3: VERDICT: CLEAN (0 open; approved base import and root-macro fix)

Reviewed head: `71ec2783edc2380c3389a71062422491506ebf39`.
Imported base: `007efbde6a59af403d0937c38361c137a8961a48`.
Tree: `4035cda3ce1eb41476d5dc6b4baeba463d8deb11`.
Previously reviewed member: `fabbf66cdaccedd942b8cedea824c763bd7bfa7a`.
Import commit: `39e298de6b8eb3722d44db3e79231224e29799f4`.
Correction receipt: `msg_plugin-w_1791838564_9ed581`.
The reviewer reads the PC brief first and follows the lead's approved import decision recorded above.
Scope: the complete applicable base delta, patch preservation, root-macro fix, retained checks, and current body.

### Merge and patch preservation

The import commit has parents `fabbf66c` and the approved base `007efbde`.
The reviewer independently runs the read-only merge-tree calculation; it reports no conflict.
Its tree is `f79e0bbbab3752b2b92dc181c8f5468efe7d7659`, exactly the retained merge commit's tree.
This verifies the automatic merge without using a branch resolution or changing the worktree.
It is a git source calculation, not a build, test, or executable base-check job.

The own diff has thirteen paths. Seven overlap the incoming base:

- `Cargo.lock`.
- `crates/botster-core-testkit/Cargo.toml`.
- `crates/botster-core-testkit/src/lib.rs`.
- `crates/botster-core-testkit/src/real.rs`.
- `crates/botster-core-testkit/src/route_client.rs`.
- `crates/botster-worker-core/DESIGN.md`.
- `xtask/src/ci.rs`.

The reviewer reads the complete changes in those shared paths and the relevant attach, stream, worker receive, and tier-selection paths.
Each own added/deleted-line sequence is preserved against the new base, excluding only the three new root-macro lines in `props.rs`.
Every changed non-own blob equals the imported base.
The full-index own diff against `007efbde` has SHA-256 `9035440b95ddd45a53f02656b6942b9d04e8d0656d70408601bb0d8e684a82b5`.

The stream counter, socket identity helper, real route, kernel query, and pure diag decoder are byte-identical to gated parent `d1aa7334`.
The frame tracker and control dispatch keep their reviewed code.
The route-client unit fixture imports the base's `WorkerClock::real()` constructor change; its frame assertions remain unchanged.
The attach path still reads the worker socket identity before Core takes the endpoint and keeps no worker endpoint copy.
The worker still decodes each complete frame in the receiving turn, then applies frames only while refusal room exists.
Its wire format still uses the four-byte length prefix that `FrameEnds` observes.
R-115's reviewer premise remains explicit in the worker design paragraph.

The base imports the report-hold and control-queue dispatches beside the route module. No own control name or guard is lost.
The base's `Report::Printed` arm does not enable the probe print-report environment or change the route query.
The testkit exports retain `sock_diag` and the two harness entries.
The imported manifest, lockfile sources, and contracts `.46` pin equal the base, apart from the preserved bolero dependency edge.
The imported mutation configuration and known-flaky file equal the base.
The latter has the approved IN-8 KSTATE-1 entry; #319 adds no known-flaky entry or exclusion.
No new facade or production test branch occurs.

### Conformance composition

All ten conformance text blobs equal the approved base byte for byte.
The own PR changes no id, reason, proof entry, deferred value, or minimum list.
This exact equality retains every removal that the base made; no old resolution restores an id.
Current counts are 388 core-pending, 38 real-pending, 692 ledger ids, and 69 minimum ids.
`am_2_one` remains a required test; `am_2_fairness` remains core-pending.
No id removal follows from this review or from a reported real-pending pass.

### CQ-RFT-5 / R319-SI-3: closed

Commit `71ec2783` adds the real `botster_test_process::built_in_test!();` invocation at the root of `tests/props.rs`.
The post-import delta is three lines: the comment, macro invocation, and separator.
Both decoder harnesses and their assertions remain unchanged.
The imported dependency provides the macro API that the old base lacked.
The macro defines the reserved root test through the existing implementation; no hand-written substitute is used.
The current default and slow tier selections include its unignored root test.

The retained current-head check runs `slow-coverage` with the macro and exits 0, covering 1100 slow-only tests.
The controlled reversal removes those three lines, then exits 1 with the missing-root finding for `botster-core-testkit::props` in both tiers.
The script restores `props.rs` and records clean status, the exact head, and completion time.
The source, positive check, and negative check close the omission. All five package findings are now closed.

### Retained current-head checks and body

Record: `~/botster-sessions/shared/core-stage1/evidence/pc-route-frames-taken/rft-merge-checks-71ec2783.log`.
Verified SHA-256: `e4176b47cd9424919f7d509a4238a67fb26864dfa2d254fd1e00309aa62a0614`.
The reviewer reads the complete record and its producing script.
The record identifies clean head `71ec2783` and reports workspace `cargo check --workspace --tests` exit 0.
That compile check precedes the recorded clean at `2026-10-12T20:42:39Z`.
The later checks report ten xtask exits 0 and both Mac/Linux-target clippy exits 0.
The local test summaries report 483/483 default, 13/13 focused slow, and strict 281 passed with zero failed and 412 ignored.
The strict command uses `BOTSTER_SEEDS=0-31`.
The existing producer filters test output and does not retain numeric test or clean exits or a complete target-writer history.
These are supporting local results. No fresh Linux mutation, fuzz, or full-gate result on this head is claimed.
The post-reversal restore has no numeric restore exit or new restored suite; its clean status and exact head are retained.

The reviewer reads the complete current body and its complete import delta.
Body SHA-256 without an added line feed: `ef3ba24b7b5a6c94dfa3cd5e047dad619a0626de027cc89eda8c53a0be9ea90b`.
Body SHA-256 with one added line feed: `89a62e146d725ceb09cd8df35efd54bde68e3b63aebf9e6767edba3d8ec008cd`.
The body names the approved merge, root fix, current checks, negative presence proof, and absence of a new full gate.
It retains the old `.43` gate, RED repair history, two FLAKY trials without catch credit, and earlier local/scratch scopes.
The parent `d1aa7334` gate remains evidence for its reviewed code only. It is not a gate on the new `.46` composition.

### CLEAN scope and next step

The complete #319 source/body/import delta is CLEAN on exact head `71ec2783`, with zero open package findings.
The premature `cf37cdc7` CLEAN remains superseded; this completed round supplies the valid package closure.
The matching complete formal integration delta verdict remains due.
The lead's train construction, any applicable train/base delta reviews, exact train gate, final evidence receipts, and Q7 remain due.
The lead's no-slot train decision applies. This verdict creates no final slot or provisional gate acceptance.
The separate contracts route fence, kernel-state list addition, and later Core pin remain PC's work.
No later head, status removal, specific retry, or completed merge is accepted.
No reviewer build, test, gate, mutation, reversal, reproduction, or executable base-check job runs.
Wait for the doorbell. Never poll.

## Train 9b — package source delta CLEAN

Train head: `3e4b3ab8172f1a04467a7969833708606a0f8580`.
Train tree: `fe7700440d2c658dbdfe223ae7b54c17f5516654`.
Train base: `2d8e8acc52eca692a089af02b4ace508d6694cc3`.
Frozen #319 member: `71ec2783edc2380c3389a71062422491506ebf39`.
Members, in merge order: #328 `6b1918c93b49f636bd9a4d0f5808ecf92909fe5c`, #319, #326
`797f090ac8fa8f182b4201d524e3add74d8d60e5`.
Lead receipts: `msg_plugin-w_1791839691_f59f4e` and replacement `msg_plugin-w_1791839934_45fad9`.
The second receipt removes #318 and replaces train 9 with train 9b. This verdict applies only to train 9b.

The reviewer reads complete matching integration Round 3 at `2d5c33472264180841ff69f3dbb3a087025e2867`.
Its exact member head, tree, base, five closed findings, evidence limits, and pending train conditions match package Round 3.
The remote integration branch contains this published record. No member finding remains open.

The reviewer compares all thirteen own patch sequences with the #319 merge in train 9b.
Every added and removed line remains exact, apart from hunk positions.
Every own path remains byte-identical between that merge and the final train head.
Own train patch SHA-256: `d676147c4637033cdfd487dc2dd6e7f55e00eb2b41e5dcd3d731b2e031039b10`.
Complete member-to-train delta SHA-256: `f22d75e5df32c90755f9432089599d49aabca6fdfcca09dec0627b5ff35e9319`.

The moved route paths change only the PTY input log representation and its callers.
The real route still encodes the probe's complete input bytes as lowercase hex before it builds the same log answer.
The testkit now uses the cached hex log. Both direct writes and released kernel bytes extend that cache with the same bytes.
The route answer retains its byte log and single unattributed segment. An empty log still has no segment.
FrameEnds, sent-byte accounting, socket identity checks, kernel queries, and route_frames_taken dispatch remain unchanged.

The new worker design topic index preserves the R-115 refusal-room checklist text.
Train 9b excludes #318's drain paragraph. The worker's route receive and refusal-room logic remain unchanged from the reviewed member.
The xtask delta replaces held-full result accounting for #328. It preserves both decoder registrations and their landing selection.
The props binary retains the real root macro and both decoder harnesses.
The dependency edges, platform paths, checked errno conversion, and unchanged decoder source remain preserved.

#319 still changes no conformance status, mutation exclusion, retry authority, or contracts pin.
Train status and exclusion changes belong to the other members and require their own package and integration reviews.
The lead reported workspace compilation and strict checks for old train 9. Those results are not relabeled as train-9b execution.
The reviewer runs no build, test, gate, mutation, reproduction, or executable base-check job.
The exact train-9b gate, combined integration review, final evidence receipt, and lead Q7 remain due.
The separate contracts fence and later Core pin remain PC's work.

VERDICT: CLEAN (0 open; #319 package source delta on train 9b only).
