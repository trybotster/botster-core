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
