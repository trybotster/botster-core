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
