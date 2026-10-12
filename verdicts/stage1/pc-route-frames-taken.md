# Integration review: route_frames_taken (#319)

## Round 1 — VERDICT: NOT CLEAN (2 open)

Reviewed head: `ab9b223611978270179cb2c2f361f3a2becfbe65`.
Source merge base: `080987829a2e2245a7ea97606020400392c974d6`.
Tree: `d926f7aea682779594933d840ad9c5a8c50f8a03`.
Gate base: `9fe97f506a1bb5cef0d935b2ba2ccf3ff9ba90ec`.
Risk: HIGH, shared testkit change.
Plan: revision 23cm, `stage1-plan.c77c2434.md`.
READY: `msg_plugin-w_1791827733_9233a6`.
Lead role correction: `msg_plugin-w_1791827904_dac628`.
Sol is the integration reviewer. 0179 is the package reviewer.
The owner's original READY reversed these roles. The later owner and package messages correct them.

The reviewer reads complete package Round 1 at `3d46120ea8d8df66c8d587d75aafa284041836e5`, `verdicts/pc-route-frames-taken.md`.
Receipt: `msg_plugin-w_1791828387_99f73d`.
That package verdict is NOT CLEAN with CQ-RFT-1 open.
This integration verdict confirms CQ-RFT-1 and adds R319-SI-1.
The reviewer also reads the complete package amendment at `1428339dae1b7b7998bd61585b2cfaf0556a9d39`.
Receipt: `msg_plugin-w_1791828599_751b9f`.
The package amendment confirms R319-SI-1 as CQ-RFT-2 and reports the same two open findings.
Both reviews now cover the same exact head, base, and tree.

### CQ-RFT-1 — MEDIUM, open: required decoder fuzz coverage is absent

Locations: `route_client.rs:84`, `real/rqueue.rs:122`, and `xtask/src/ci.rs:72`.
`FrameEnds::push` decodes the client's four-byte frame prefixes.
`diag::parse` decodes binary message headers and attributes from `unix_diag`.
Neither new byte decoder has a bolero harness.
The testkit has no entry in `HARNESSES`.
The PR changes neither that list nor the fuzz command.
The gate reports `fuzz: the diff changes no crate with a decoder harness` at line 6479.
The fuzz step reports PASS after 0.0 seconds. No fuzz harness runs.

Plan section 8, step 9 requires one harness per decoder.
Each harness must run capped property cases with a pinned seed in the default tier.
Each selected landing harness must run for 60 seconds on the pinned nightly.
Unit tests and mutation catches do not replace these checks.
`real` currently requires the `slow` feature. The selected command must compile the diag harness.

Required closure:

1. Add bounded bolero harnesses for both new byte decoders.
2. Cover incomplete bytes, split writes, and complete-frame counts for `FrameEnds`.
3. Cover malformed, truncated, and valid diag replies.
4. Register the harnesses and compile their paths under the landing command.
5. Supply actual default property and landing fuzz evidence on the corrected head.
6. Retain any discovered crash input as a regression case.

### R319-SI-1 — MEDIUM, open: an invalid errno can panic during decoding

Location: `crates/botster-core-testkit/src/real/rqueue.rs:135`.
The `NLMSG_ERROR` arm reads an arbitrary `i32` at offset 16.
The arm then calls `io::Error::from_raw_os_error(-errno)` without checking the negation.
A 20-byte message can declare length 20, type 2, and `i32::MIN` at offset 16.
The decoder reaches this negation without another validation condition.
Negation of `i32::MIN` overflows when overflow checks are enabled.
The decoder therefore panics instead of returning an error for that invalid reply.
R-93 condition (b) and the module's stated failure behavior require a harness error.

This finding concerns the decoder's invalid-input behavior. It does not claim that the kernel produces this errno.
The existing error test covers only `-2`, not this boundary.
The reviewer establishes the path from source and runs no reproduction.

Required closure:

1. Reject an invalid errno before conversion, with range validation or checked negation.
2. Retain the `i32::MIN` error message as a regression input.
3. Show that the corrected decoder returns an error without a panic.

Source notices: owner `msg_plugin-w_1791828457_362af6`; package `msg_plugin-w_1791828457_7b7b67`.

### Source and boundary review

The reviewer reads all seven changed files, the R-93 serving-point conditions, and the complete package verdict.
The diff has 744 additions and one deletion.
The reviewer also reads the attach path, existing stream accounting, and the worker's receive/apply path.

The testkit reads the existing `taken[side]` stream counter under its existing lock.
The real tier counts bytes that the client socket accepts. It excludes bytes that remain in `unsent`.
The real tier subtracts the worker socket's unread bytes with `checked_sub`.
An unread count greater than the sent count returns a harness error.
`FrameEnds` records submitted bytes, but counts only frame ends within the taken-byte prefix.
The byte walk ends after the supplied byte count, including incomplete prefixes and bodies.
The source tests cover split prefixes, split bodies, empty bodies, partial counts, and bytes that remain in the client.

The attach path captures the worker socket identity before Core takes the endpoint.
The route retains its client endpoint and captures that endpoint's identity on the first control call.
The Linux query requests the worker inode's peer and receive queue, then checks the peer against the client inode.
The query takes one nonblocking reply. It adds no worker endpoint copy or worker-memory query.
The macOS query uses absolute `/usr/sbin/netstat`, `LC_ALL=C`, and the existing cleanup deadline.
The parser requires the six named header columns and exactly one worker-address/client-connection row.
Call, timeout, absent-socket, peer, and parse errors return harness errors, except for R319-SI-1's invalid errno path.

The worker decodes received frames before application. `apply_route` can defer application when refusal room is absent.
The new design paragraph retains R-93's premise: no route gate or accept limit, and no lowered route queue bound.
The separate contracts PR must check that premise before any transcript uses this fence.
That PR also owns driver dispatch, the controls row, fake behavior, and transcript fences.
#319 moves no contracts pin and changes no conformance list.
Both AM-2 ids remain pending. No removal follows from this review.
The worker source remains unchanged; only its design document changes.
No production test branch, new worker wait, or new worker lock appears.
No additional source finding is identified.

### Gate evidence

Log: `~/botster-sessions/gates/botster-core-stage1-pc-route-frames-taken-ab9b2236-pool-20261012-103338-83559.log`.
Verified SHA-256: `87d4fc09cb7b8e84f93b384ce087092b684014b48e045fe5ca8b8cffe43b2512`.
The retained `gate-client-ab9b2236.out` is byte-identical to this log.
The log names the exact candidate head, four CPUs, MS-A1, and gate base `9fe97f50`.
Its list check names source merge base `08098782` separately.
This initial gate supplies no current-base final import receipt.

All ten steps report PASS, with total CI time 1263.5 seconds.
The job exits zero after 1273 seconds. The wrapper exits zero after 1274 seconds.
Default tests pass: 1937, with 431 skipped, in 11.848 seconds.
Normal slow tests pass: 627, with 2070 skipped, in 40.189 seconds.
The unmutated Stage 2 baseline passes 627 tests, including ten slow tests, with 2830 skipped, in 39.775 seconds.
Both strict summaries report 261 passed, zero failed, and 430 ignored.

Stage 1 tests 86 mutants: 15 caught, 71 missed, zero timeout, and zero unviable.
Stage 2 tests those 71: 67 caught, four unviable, zero missed, and zero timeout.
The final result is 82 caught and four unviable, with zero missed or timeout.
The reviewer compares all raw catch identities with all citations. Both sets have 82 unique entries and match exactly.
Every citation has a normal PASS record. Every Stage 2 citation also has an unmutated baseline PASS record.
The log has no runtime FAIL, FLAKY, RETRY, INCONCLUSIVE, or TIMEOUT result.
The four unviable mutants supply no behavioral catch.
The Linux peer-check mutant has a catch through `bytes_that_wait_in_the_client_are_not_taken`.
The empty fuzz selection remains CQ-RFT-1. This review does not treat its PASS label as decoder fuzz evidence.

### Other evidence and body limits

Evidence directory: `~/botster-sessions/shared/core-stage1/evidence/pc-route-frames-taken/`.
The reviewer reads the current local log and local/per-seed/reversal scripts, and audits retained probe, reversal, and focused-mutation results.
The Linux probe reports unread counts 0, 100, 70, and 0, with the correct peer inode.
The worker's own read-side query agrees. The socket disappears after worker exit, and the probe exits zero.
The Mac probe reports the same counts in matched `Address` and `Conn` rows on macOS 27.0.1.
These probes establish the ruled access paths. They do not execute this final Core head.

The current local record identifies clean head `ab9b2236` before checks at `2026-10-12T17:25:52Z`.
It reports ten xtask exits zero, both clippy exits zero, 442 default passes, 13 focused slow passes, and strict 261/0.
The script omits numeric test exits and the clean exit. Those omissions limit this supporting record.
The exact Linux gate independently supplies its reported test results.

The Mac reversal record identifies earlier head `22888a13`.
The baseline passes two default and ten slow tests.
All thirteen applicable changes cause test failures with exit 100.
The Linux peer-check change is not compiled there; the current Linux gate supplies its mutation catch.
The script restores source and prints status. It does not run another restored suite.
The focused Linux record identifies earlier head `70679e8a`: 30 mutants, 28 caught, two unviable, and exit zero.
These earlier records retain their original head and platform scopes.

The per-seed record uses Core `22888a13` and scratch contracts `1baa18ed` through a temporary path override.
It records fenced `am_2_one` PASS on both tiers for seeds 0–31, with numeric exits zero.
The real run takes 8.41 seconds. The script restores the manifest and lockfile.
This record accepts neither the currently pinned transcript nor a future contracts patch.
The body preserves three earlier RED gates and identifies their fixes without changing their verdicts.
Body SHA-256 without an added line feed: `a69a136905ffaf4c5b38623cc61266ed1e0bcc916724e57a5281986a03928e72`.
The body's Prior art note names the old unsafe reader-side `FIONREAD` access and explains the new kernel query.

### Next step

Both findings must close before integration CLEAN.
A corrected head requires a delta review, actual decoder evidence, and the complete matching package verdict.
The current-base final slot, final import review, final gate, matching final receipts, and lead Q7 remain due after both CLEANs.
This review accepts no later head, pin move, conformance removal, retry, or merge.
The reviewer runs no build, test, gate, mutation, reversal, reproduction, or executable base-check job.
Wait for the correction doorbell. Never poll.

## Round 2 — VERDICT: NOT CLEAN (1 open; repair and text delta)

Reviewed head: `fabbf66cdaccedd942b8cedea824c763bd7bfa7a`.
Source merge base: `080987829a2e2245a7ea97606020400392c974d6`.
Tree: `56a6056cbe91f05850e81ab724db39aed3b5992f`.
Gated parent: `d1aa733480bbc9ee5449e70c6b34939fa18065e9`.
Parent tree: `6b70e352c35172291a483c8445233ff96d2bc4fd`.
Gate comparison base: `2683de7ccbb1360d124422b3b6a2dd196b046c7e`.
Repair READY: `msg_plugin-w_1791836503_624f59`.
Risk remains HIGH. The reviewer inspects the complete repair from the Round 1 head and the final design correction.

The reviewer reads complete package Round 2 at `80d96b69804fda10d7f13eaa26f8bee4aa6ea462`.
File: `verdicts/pc-route-frames-taken.md`. Receipt: `msg_plugin-w_1791837246_9e7ceb`.
Both reviews retain one MEDIUM finding, CQ-RFT-5 / R319-SI-3, on the same exact head, base, and tree.
Package commit `cf37cdc788fbaeb669d4f2d0b7d0aed2d478df5c` published premature CLEAN before this finding arrived.
The package reviewer supersedes that CLEAN and reports that no terminal CLEAN from it reached the lead.

### CQ-RFT-1 — MEDIUM, closed: both decoder harnesses now run

The pure diag decoder moves to unconditional, public, doc-hidden `src/sock_diag.rs`.
Its OS exchange stays in `real/rqueue.rs`. The Linux exchange and macOS netstat paths keep their prior behavior.
`FrameEnds` exposes its methods for the external property binary. The Core facade does not change.
The testkit adds the existing workspace bolero dev dependency. The lockfile adds only its dependency edge.
The existing landing harness list registers both decoder tests. This adds no new xtask check family.

Each property test caps cases at 256 and uses the existing tier seed environment.
`frame_ends_decoder` compares chunked and whole-input results with a separate reference decoder.
It checks last-byte boundaries and constructs up to 64 complete frames from input-derived body lengths.
`sock_diag_decoder` accepts arbitrary bytes without a panic and checks structured peer and unread values in either attribute order.
It rejects the wrong inode and a truncated reply. Every case retains the `i32::MIN` error reply.
Both tests have normal default PASS records and completed landing fuzz runs in the parent gate below.

The earlier repair gate on `86fdb2096e075f94a71d3d6b014710d685ffd1b9` remains RED.
It misses the replacement of `FrameEnds::ends` with an empty vector. Fuzz does not run in that RED gate.
The d1 repair checks `ends()` directly and adds complete generated frames to the property harness.
The new gate catches the former survivor through the frame-boundary unit test.
This is a changed-source gate. The review grants no same-head retry.

### R319-SI-1 / CQ-RFT-2 — MEDIUM, closed: errno conversion is checked

The decoder uses `checked_neg()` and requires a positive converted errno before creating the OS error.
Invalid `i32::MIN` and zero replies return errors. A valid `-1` reply retains its expected OS error.
The unit regression passes in normal slow tests and the unmutated Stage 2 baseline.
The default property test and landing fuzz run retain the invalid boundary reply.
The supplied Mac reversal removes checked conversion and fails both the property test and unit regression with exit 100.
Another reversal admits zero and fails the unit regression with exit 100.
The source, actual gate, and retained negative results close the invalid-input finding.

### CQ-RFT-3 — LOW, closed: body reports FLAKY outcomes and correct AM-2 status

Two Stage 2 trials of `conf::am_2_one_admission_point_orders_all_sources` fail, then pass on the permitted retry.
They occur under `sock_diag.rs:64:5`, `i32_at -> Some(1)`, and `sock_diag.rs:85:32`, `>` replaced by `<`.
Neither FLAKY trial earns catch credit.
Both mutants instead have valid catches through `real::rqueue::slow_tests::the_diag_reply_gives_the_peer_and_the_unread_bytes`.
Those catches have normal and unmutated baseline PASS records. The corrected body reports both outcomes and the separate catching test.

The owner attributes the FLAKY trials to the existing unfenced cross-source order under R-93.
Source inspection shows that the pinned transcript has no `route_frames_taken` fence and does not use the diag helper directly.
This source evidence supports a separate ordering issue. It does not establish the trial failure's cause from the log alone.
The body names PC's pending contracts fix on `stage1/pc-route-frames-taken`, reported head `bdad2be3`.
Its reported running gate and unopened PR supply no completed evidence here.
The body separates that work from merged R-111 contracts #100 and the later Core pin under rule 23am.

Round 1 incorrectly says that both AM-2 ids remain pending. This paragraph corrects that statement without changing its history.
`conf::am_2_one_admission_point_orders_all_sources` is absent from both pending files and remains a required test.
`conf::am_2_fairness_bound_across_routes` remains in `core-pending.txt`.
The repair changes no conformance file and approves no status removal.
The reviewed head still pins contracts `.43`, commit `c69a3190b4d66e84401006e9d73788ff0e837f56`.

### R319-SI-2 / CQ-RFT-4 — LOW, closed: the refusal-room premise is a reviewer check

R-115 stops the proposed R-93 premise lint extension. The conditions remain reviewer checklist item 6.
The earlier repair body and worker design paragraph incorrectly assign those conditions to the order lint.
Commit `fabbf66c` corrects the design paragraph. The complete corrected body makes the same correction.
Both require no route gate or accept limit from the first frame to the fence, with the default `route_queue_bytes`.
The existing R-93 order lint stays unchanged. No new lint or check enters this repair.
R-115 supersedes Round 1's requirement that the contracts PR add a premise check.

### R319-SI-3 / CQ-RFT-5 — MEDIUM, open: the new property binary lacks the required root macro

Location: `crates/botster-core-testkit/tests/props.rs`.
The new binary contains both decoder tests, but no `botster_test_process::built_in_test!();` invocation.
Current v1 `3e655eff46d36f8c337339a1a17d401c8c662594` enforces root-test presence in `xtask/src/slow_coverage.rs:195`.
Both default and slow binary lists must contain the selected, unignored `this_test_binary_was_built_in_this_worktree` test.
All five existing testkit integration files use the macro on that base.
The new binary therefore lacks the existing B13 verification for its build root.
This is an existing requirement. Its closure adds no xtask check and needs no R-115 check exception.

Required change: add the existing `botster_test_process::built_in_test!();` macro and retain both decoder harnesses.
Supply the corrected exact head and applicable root-test and coverage evidence.
Do not substitute a handwritten test with the reserved name.

The reviewed source base predates both the coverage enforcement and the macro API.
A source search finds no `macro_rules! built_in_test` in the reviewed head's `botster-test-process` crate.
The current v1 crate defines it in `src/built_in.rs:127`.
Thus adding the invocation requires a dependency import from newer v1 before it can compile.
The old-base parent gate cannot detect this current-base requirement.
Owner receipt `msg_plugin-w_1791837079_696676` confirms the missing API and asks when the v1 import can occur.
The owner cites the lead's rule against a final import before SLOT.
Pair-common also permits an earlier import when the build needs it.
The lead must resolve that import timing before this finding can close. This review grants neither an import nor provisional CLEAN.

Source notices: owner `msg_plugin-w_1791837054_bde8c7`; package `msg_plugin-w_1791837054_078357`.

### Independently audited parent gate

Log: `~/botster-sessions/gates/botster-core-stage1-pc-route-frames-taken-d1aa7334-pool-20261012-125446-96535.log`.
Verified SHA-256: `c42efaa14229841b83d677f915c591ef7f2042c2e8d01f7491a87259cfcb0462`.
The retained `gate-client-d1aa7334.out` is byte-identical. The canonical log has 7107 lines.
The header identifies exact d1, MS-A1, four CPUs, both caps 4, and comparison base `2683de7c`.
The source merge base remains `08098782`. This is repair evidence, not a final current-base receipt.

All ten steps PASS. CI takes 1474.5 seconds.
The job and wrapper exit zero after 1571 seconds. The job queues 88 seconds and runs 1483 seconds.
Default: 1939 unique passes, 431 skipped, 11.954 seconds. The maximum test time is 1.167 seconds.
Normal slow: 627 unique passes, 2072 skipped, 40.476 seconds.
Unmutated Stage 2 baseline: 627 unique passes, 2832 skipped, 40.977 seconds; complete step 51.2 seconds.
Each pass ordinal set and denominator matches. Both strict summaries report 261 passed, zero failed, and 430 ignored.
The list report has 406 pending ids, 43 real-pending ids, 691 ledger ids, and minimum 69.
The ignored real-pending run has 35 passes and zero failures within the retained 43 ids.
Held-full has 96 ignored, zero failures, and takes 22.14 seconds. Its three budget ids use 32 seeds.

Stage 1 has 92 unique mutants: 46 caught, 43 missed, three unviable, zero timeout, 285.0 seconds.
Stage 2 tests exactly those 43 misses: 42 caught, one unviable, zero missed, zero timeout, 666.9 seconds.
All 88 raw catches match unique citations with normal PASS records.
All 42 Stage 2 citations also have unmutated baseline PASS records.
The two FLAKY trials above earn no catch credit. The unviable replacements earn no behavioral credit.
No other runtime FAIL, TIMEOUT, SIGSEGV, SIGABRT, or panic result appears in the gate.

Both new fuzz harnesses run for a configured 60 seconds and complete after 61 seconds.
`frame_ends_decoder` completes 3195602 runs. `sock_diag_decoder` completes 5847439 runs.
Neither reports a crash. The complete fuzz step takes 325.6 seconds; the mutant step takes 1005.8 seconds.
This `.43` gate executes neither the future route fence nor current v1's root-presence enforcement.

### Supporting evidence, body, and delta limits

The repair from ab9 to d1 changes eight paths, with 314 additions and 129 deletions.
Full-index diff SHA-256: `172dec85cc5040b390c66e06bab70a6fb5ef0bd89b6554dc8a67c77aeaaf0f0b`.
The d1-to-fab correction changes only the worker design paragraph.
Diff SHA-256: `f170f60ce05909c0c081aee834f81d227e2167429ada3236241d503078661de8`.
All conformance blobs remain unchanged from ab9 to fab.
Code equality carries the parent gate's unchanged-code evidence. It does not claim execution on fab.

The reviewer reads the complete corrected body and all body corrections.
Body SHA-256 without an added line feed: `a0c173944b0ea479572145c236f38c3c1a0aaaf3426b3838d5f77ecaeaa9175e`.
The body preserves RED repair history, FLAKY outcomes, and separate contracts and pin prerequisites.

The retained `red-on-revert-86fdb209.log` has SHA-256 `80bcb727cb3609760bcffdec27382f288f8f47eb03e6b6b8a65a8f0ed0724227`.
It identifies exact 86, Darwin 27.0.0, and the clean marker at `2026-10-12T18:16:46Z`.
The baseline passes four default and twelve slow tests, with numeric exits zero.
Fifteen Mac-applicable reversals each fail a named test with exit 100. The Linux-only peer reversal supplies no Mac negative proof.
The script prints diff statistics and retains replacement patterns. It does not retain each complete raw patch.
It restores source and prints clean status, exact 86, and time. It records no numeric restoration exit or restored suite.
The Linux gate independently supplies the peer-check mutation catch.

The `ends-mutant-d1aa7334.log` hash is `de9a1016972b4441ed847309d8f899b01817231ce1a8844ac95959354e85b865`.
Its five lines show failed unit and property tests, but omit head, command, scratch source, numeric exit, and restoration.
This owner-supplied fragment is supplementary. The exact gate supplies the accepted catch.

The `rft-local-checks-d1aa7334.log` hash is `feb034eccf65f5b346041df470fcf6a407ec6734ae4d18970eb3aaef5a2a0529`.
It identifies exact d1, Darwin, and a clean marker at `2026-10-12T19:21:00Z`.
It reports ten xtask exits zero, both clippy exits zero, default 444/444, focused slow 13/13, and strict 261/0.
Its producer omits numeric clean and test exits. The record supplies no complete target-writer history.
Local Mac strict, slow, and timing results remain supplementary. Earlier probe and scratch-contracts records retain their original scopes.

### Next step

Four findings close. R319-SI-3 / CQ-RFT-5 remains open and prevents both integration and package CLEAN.
The lead's import decision, corrected head, applicable evidence, and matching package delta review remain due.
After both CLEANs, current-base compilation, final SLOT, base review, exact final gate, final receipts, and lead Q7 remain due.
The separate contracts fence and later Core pin remain PC's work under rule 23am.
No reviewer build, test, gate, mutation, reversal, reproduction, measurement, or executable base-check job runs.
Wait for the decision or correction doorbell. Never poll.

VERDICT: NOT CLEAN (1 open).

### Lead decision after publication: import approved; finding remains open

Lead receipt: `msg_plugin-w_1791837657_ac3742`.
The lead approves option b: PC imports current v1 once now and adds `built_in_test!()` in the same step.
The lead states that the train model has no SLOT. This replaces the SLOT timing assumed in this round's next step.
The lead reports that train 7b landed and names current v1 `007efbde6a59af403d0937c38361c137a8961a48` for the import.
PC must supply the resulting exact head before FINAL-READY. Sol and 0179 must review the complete applicable delta.
The decision resolves import timing. It supplies no corrected source or execution evidence and does not close R319-SI-3 / CQ-RFT-5.
VERDICT: NOT CLEAN (1 open).

## Correction inspection — current-base import and root macro; matching package delta remains due

Corrected head: `71ec2783edc2380c3389a71062422491506ebf39`.
Tree: `4035cda3ce1eb41476d5dc6b4baeba463d8deb11`.
Imported v1 and current source base: `007efbde6a59af403d0937c38361c137a8961a48`.
Previous reviewed head: `fabbf66cdaccedd942b8cedea824c763bd7bfa7a`.
Correction receipt: `msg_plugin-w_1791838564_4b8ba0`.
The reviewer inspects the authorized import, the root macro correction, complete body, and retained check and reversal records.
The complete matching package delta remains due. This inspection completes no integration CLEAN round.

### Import identity and preservation

Merge `39e298de6b8eb3722d44db3e79231224e29799f4` has exactly the previous reviewed head and named v1 tip as parents.
Its tree is `f79e0bbbab3752b2b92dc181c8f5468efe7d7659`.
The reviewer independently runs static `git merge-tree --write-tree` on those parents and obtains that exact tree.
The remerge diff is empty. No manual resolution enters this import.
The two parents have one merge base, `080987829a2e2245a7ea97606020400392c974d6`.
The next commit adds exactly three lines to `tests/props.rs`: the root macro, its comment, and a blank line.
No other post-import edit enters this corrected head.

Import full-index diff SHA-256: `abeea40f5aa6e65bd6d33cdc883c08d46524f9dcc3120d1152e65f1fde5c78f3`.
Macro correction diff SHA-256: `4c8ee5a3697dd7bc67c03f57f6497134e8b86431b6e87960f3991d3a49cd3be2`.
Current-base own diff SHA-256: `9035440b95ddd45a53f02656b6942b9d04e8d0656d70408601bb0d8e684a82b5`.
The current-base own diff has thirteen paths, 932 additions, and one deletion. The Git whitespace check passes.

The reviewer compares all thirteen own edit sequences against the previously reviewed patch at its original source base.
Every added and removed line remains exact, excluding hunk positions and the three new macro lines.
The complete net, real-route, real-rqueue, real-holders, and sock_diag blobs match the previous reviewed head exactly.
Both property harnesses and their helpers also match after removal of the three macro lines.
The route_client blob changes only its test constructor to the imported `WorkerClock::real()` API.
FrameEnds behavior, route accounting, checked errno conversion, and kernel access paths remain unchanged.
The unconditional sock_diag export, both harness registrations, existing bolero dependency, and lockfile dependency edge remain present.
The imported worker report-order design paragraph coexists with the corrected R-115 refusal-room paragraph.

### R319-SI-3 / CQ-RFT-5 — MEDIUM, closed in the corrected source

The new binary invokes `botster_test_process::built_in_test!();` at its root.
The invocation uses the existing imported macro. It defines no handwritten substitute with the reserved test name.
The reviewer reads the complete macro definition and its build-root assertion interface.
The macro expands the caller's compile-time package and manifest directory and creates the selected, unignored libtest test.
The existing slow filter explicitly selects that test name for every binary.
The macro definition, slow-coverage implementation, and tier filter match imported v1 exactly.
The source thus satisfies the existing B13 requirement without adding a new xtask check.

The corrected-head record reports slow-coverage exit zero, with 1100 slow-only tests checked.
The retained producer then removes exactly the macro, comment, and blank line from props.rs.
The reversal reports both default and slow `botster-core-testkit::props` binaries missing the build-root test.
It reports two violations and slow-coverage exit 1. This fails the required presence condition rather than a setup condition.
The producer restores props.rs and records blank status, exact corrected head, and time.
It retains no numeric restoration exit or new restored suite. This review infers neither.
The source, corrected coverage result, and targeted negative result close the root-macro finding.

### Current-base lists and contracts

All nine conformance text sets satisfy the three-way import formula against the sole merge base.
Each final set equals the previous reviewed set minus v1's removals, plus v1's additions against that merge base.
All nine final conformance blobs also equal imported v1 byte for byte. No status change belongs to #319.
The reviewed head retains the imported `.46` manifest exactly.
The existing known-flaky list also equals imported v1; #319 adds no entry or retry authority.
The control still precedes its separate contracts fence and later Core pin under rule 23am.
This inspection accepts neither a future contracts head nor execution of that future fence.
The corrected AM-2 status distinction and the earlier two FLAKY outcomes keep their Round 2 dispositions.

### Supplied corrected-head checks and limits

Log: `shared/core-stage1/evidence/pc-route-frames-taken/rft-merge-checks-71ec2783.log`.
Verified SHA-256: `e4176b47cd9424919f7d509a4238a67fb26864dfa2d254fd1e00309aa62a0614`.
The complete record has 46 lines and names exact corrected head with clean status at `2026-10-12T20:42:05Z`.
Producer: `rft-merge-checks.sh`, SHA-256 `511c4cb86619d2579bd3955e128dc9bbf36ca26b63199683f0c4fddbd8322ec8`.
Called producer: `rft-local-checks.sh`, SHA-256 `440fa680dade11edfdb2820eb8261d6a30b7963f71b48bc0025a8a80e814f4bd`.
The reviewer reads both complete producers. They unset RUSTUP_TOOLCHAIN and set both concurrency caps to four.

The first workspace check reports exit zero and finishes after 19.47 seconds.
That command is `cargo check --workspace --tests`, without `--locked`, and precedes the subsequent clean marker.
The record supplies no complete prior target-writer history for that first check.
The called producer reports its clean marker at `2026-10-12T20:42:39Z` on Darwin 27.0.0, macOS 27.0.1.
It does not retain the numeric clean exit. The previous evidence limits therefore remain explicit.
All ten named xtask checks report exit zero. Both clippy commands report exit zero, with warnings denied.
The Linux target check compiles the netlink path; it is not Linux runtime evidence.

The default testkit summary reports 483/483 passes in 2.925 seconds.
The focused slow record has thirteen named passes, zero failures, and 575 skipped in 0.450 seconds.
The strict summary reports 281 passed, zero failed, and 412 ignored in 6.37 seconds.
The producer explicitly sets strict seeds 0–31. It omits numeric test exits and filters the test output.
Local Mac strict, slow, and timing results remain supplementary under pair-common.
The focused slow selection does not run the root test itself; slow-coverage proves its selection in the full slow tier.
The final exact-head train gate must execute the selected root tests and the other required steps.

The root-macro reversal occurs after these checks and records slow-coverage exit 1.
Its source restoration ends at `2026-10-12T20:55:16Z`. The script retains the removal pattern and diff statistics, not the complete raw patch.
The independently inspected three-line commit and matching omission diagnosis establish this reversal's source scope.
No new full gate, mutation run, or fuzz run is supplied for this corrected head.
The lead's handoff permits one import plus the macro, local checks, and both deltas; the train gate is the merge gate.
Lead receipt `msg_plugin-w_1791837657_ac3742` supplies the updated train 7b import tip and replaces SLOT timing.
The completed d1 gate retains its exact old head, unchanged-decoder evidence, FLAKY labels, and other recorded limits.
It is not relabeled as corrected-head or merging-gate execution.

### Body and next step

The reviewer reads the complete current body and its complete delta from the previous reviewed body.
Body SHA-256: `ef3ba24b7b5a6c94dfa3cd5e047dad619a0626de027cc89eda8c53a0be9ea90b`.
The new section names the authorized import, three-line macro correction, corrected-head local checks, and negative coverage result.
It preserves the earlier RED history, two FLAKY trials, exact d1 gate scope, and separate contracts prerequisites.
Zero source or body findings remain, including LOW.
The complete matching package delta remains due before the integration CLEAN round.
The applicable train composition review, completed exact merging-head gate, final receipt, and lead Q7 remain due afterward.
No corrected-head gate, future pin, specific retry, completed merge, or final train acceptance is granted here.
The reviewer changes no product code and runs no execution job.
Wait for the complete matching package delta doorbell. Never poll.

## Round 3 — approved current-base import and root-macro correction CLEAN

Head: `71ec2783edc2380c3389a71062422491506ebf39`.
Tree: `4035cda3ce1eb41476d5dc6b4baeba463d8deb11`.
Imported base: `007efbde6a59af403d0937c38361c137a8961a48`.
Previously reviewed head: `fabbf66cdaccedd942b8cedea824c763bd7bfa7a`.
Scope: the authorized v1 import, preserved own changes, root-macro correction, current body, and supplied correction evidence.

The reviewer reads complete matching package Round 3 at `27761dc18856339efee7a4280cc5759b2869ff27`.
File: `verdicts/pc-route-frames-taken.md`. Receipt: `msg_plugin-w_1791839369_66950c`.
It is CLEAN on the exact same head, tree, and imported base.
It agrees with the complete independent correction inspection at `f9164502ed0e0252a16e0cf78ee0c2bf47da54ac`.
That inspection's source conclusions, positive and negative presence evidence, body hash, and evidence limits remain applicable.

All five findings close. R319-SI-3 / CQ-RFT-5 closes through the existing root macro and matching coverage results.
Zero source, body, or import findings remain, including LOW.
The inspected automatic merge preserves all thirteen own edit sequences apart from the required three-line macro insertion.
The conformance files retain the imported base. No status removal, new pin, or retry authority enters this PR.
The earlier decoder fuzz and mutation results remain scoped to their exact d1 head and unchanged decoder code.
The new local compile and coverage results remain scoped to this corrected head, with all recorded limits.
No full gate on this `.46` composition or final train acceptance is claimed.

The lead's train composition review, applicable member deltas, completed exact merging-head gate, final receipt, and Q7 remain due.
The separate contracts route fence, kernel-state list addition, and later Core pin remain PC's work under rule 23am.
The no-slot train decision applies. This CLEAN creates no final slot and grants no future head or merge.
The reviewer changes no product code and runs no execution job.

VERDICT: CLEAN (0 open; corrected source, body, and authorized import only)
