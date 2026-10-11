# PR #230 — P4a allocation controls

## Source inspection — 2026-10-10 — Exact-head evidence pending

Inspected head: `e6b67617f2c48a4a6b7974e6fa0f5ed690df71bf`.
Base: `cf3bcaa93faa240b418fa2248884a96581fd82c3`.
READY receipt: `msg_plugin-w_1791692738_39fb28`.
Scope: all twelve own changed paths, the allocator boundary, the unsafe exception, and conformance accounting.
This is a HIGH PR under BUILD rules 1 and 3. The exact-head gate still runs. No package artifact is supplied yet.
This inspection completes no review round and grants no CLEAN verdict.

### SI-1 / F116 LOW — OPEN: required Prior art note is absent

The supplied PR body has no Prior art note. `pair-common.md` requires that note on every PR.
Required change: name the relevant old Core code at `72b2e3354ffc291e39f9a5d7eb2f9c5fcbb5e79c` and explain reuse or rejection.
Also explain the hand-written allocator wrapper and the generalized unsafe exception check.
Old Core has CountingAllocator wrappers in its terminal inventory, owner lookup, and plugin completion tests.
It also has `crates/botster-core/src/runtime/plugin_process/capped_allocator.rs`.
I found these paths through source search. This does not accept them as reusable code.
A body-only correction needs no new source head.

### SI-2 / F115 MEDIUM — OPEN: the R-52 subject proof can measure the client alone

R-52 requires a positive proof that the worker's large frame allocation appears in the calling thread's peak.
That proof must fail if the testkit moves the worker to another thread.
The current proof is `a_frame_that_the_in_process_route_reader_takes_is_in_the_calling_threads_peak`.
After its final window reset at `alloc_controls/tests.rs:200`, it calls `route.read(1 << 20, ...)` eight times at line 202.
`TestkitRoute::try_read`, `route_client.rs:210`, first allocates `vec![0u8; max.max(1)]`.
That allocation is 1 MiB, which equals LARGE. It occurs on the calling thread before `workers.run_ready` at line 244.
Thus `peak >= LARGE` at test line 205 can pass without any worker allocation in that thread.
The assertion that ROUTE_STREAM_BYTES is below LARGE bounds the stream queue. It does not bound the client's read buffer.
This is a source counterexample. I ran no reproduction job.

Required change: exclude client allocations of LARGE from the measured window and require the worker's large frame allocation through public boundaries.
Retain R-52's positive and negative conditions. Keep the production worker free of test instrumentation.
The package reviewer independently reports the same finding in `msg_plugin-w_1791692805_93643e` and confirms it in `msg_plugin-w_1791692828_fa266e`.
Both reviewers identify this as F115 MEDIUM. A passing gate does not close the defective proof.

### Source and boundary conclusions

The lead permits this second unsafe exception in its 2026-10-10 option A ruling, recorded in `handoffs/core-lead.md`.
The testkit's lint table differs from the workspace only by unsafe_code = deny instead of forbid.
The only allow attribute is on the GlobalAlloc implementation for PeakAllocator in its named file.
The four allocator methods forward the original arguments to System. Safe code records alloc, alloc_zeroed, and the new realloc size.
The const thread-local Cell has no destructor and needs no allocation. Its update uses no lock or wait.
The installed flag uses relaxed atomic operations. The allocator uses try_with for thread teardown.
The measurement lives in the testkit. Production machines receive no test branch or allocator hook.
Only the testkit unit-test binary and the conformance binary install the global allocator.
Other linked binaries retain their allocator. The separate no_peak_allocator target checks Unsupported and absent control support.
The controls reject unknown arguments and retain all other registered controls.

The generalized unsafe check retains exact manifest, source file, and item identifiers for both allowed exceptions.
It rejects other manifest differences, other lint entries, and other unsafe_code identifiers, including nested and macro attributes.
Its command reads both exception manifests. The anchor's prior exception and the Ghostty binding's separate exception remain intact.
The mutation configuration changes only cited test names and their reason. It adds no exclusion.

I read pinned R-37 and R-52 at contracts `b161db87`, the allocation control rows, and the DP-3 transcript.
The transcript compares two over-limit lengths, 4097 and 0x7FFFFFFF, and requires the larger peak to be no greater.
Both cases require frame_too_large and the route close. It observes the in-process worker through the same testkit code path.
Core A20-1 permits this one id as testkit-proven. The real runner already lists that exception without running it.
This PR removes only `conf::dp_3_frame_limit_checked_before_allocation` from core-pending.
It retains the held A20 list proof and updates only the description of its remaining RealCoreHarness obligation.
No contract pin, real-pending ID, or public production API changes.
SI-2 prevents acceptance of the required per-thread subject guard, despite the transcript's current passing result.

### Source merges and supplied evidence

I compare all fifteen source merges through Git objects. Fourteen automatic trees match their published merge trees.
Only `11193fcb` has a conflict. Its resolution changes comment lines in core-pending and preserves the v0.1.27 text and own A20 note.
No executable merge-check job runs. The supplied base-merge-check receipt reports PASS at conditions (1) through (4).
The final head tree equals the earlier gate head a51c8969: `90430b5b024eb96c543e1730938539662b4c167c`.
The final merge adds v1 cf3bcaa9 without a file change. The accepted #227 input and later v1 imports remain intact.

Earlier gate: `~/botster-sessions/gates/botster-core-stage1-p4a-alloc-peak-a51c8969-pool-20261010-210529-59325.log`.
I read its header, result records, conformance reports, and allocator mutation outcomes through static parsing.
All ten steps pass in 705.2 seconds. Default: 1596 passed in 14.094 seconds. Slow: 408 passed in 106.456 seconds.
Both testkit reports show 206 passed and zero failed. Strict pending runs 373/394, with 21 real-only IDs and two held IDs.
Testkit minimum is 55/69. Real minimum is 35/68, real-accepted is 36/69, and real-pending remains 83 with zero passing IDs.
The real report names the DP-3 id as testkit-proven. It does not claim a real allocator proof.
Stage 1 records 175 distinct outcomes: 171 caught and four unviable, with zero misses and timeouts, in 334.7 seconds.
Stage 2 skips. The job and wrapper each exit zero after 1110 seconds, including 333 queued seconds and 777 running seconds.
This earlier gate has the same source tree. It does not replace the pending final-head gate.

I read all four retained correction logs and the unsafe-exception reversal script without running them.
Removing the conformance allocator yields Unsupported for alloc_peak; the unmodified id passes.
Three unsafe-check weakenings fail the corresponding item, file, or manifest tests; the unmodified check passes seven tests.
The separate no-allocator binary fails both installed/supports -> true replacements and passes unmodified.
These records retain result summaries and selected failed assertions. They supply no exact-final-head reversal header.
They do not prove the R-52 positive subject condition identified by SI-2.

Two findings remain open. I sent both to P3 and the package reviewer.
The exact-head gate, package artifact, and corrections remain pending. No formal round is complete.
I changed no product code and ran no build, test, gate, mutation job, measurement, reversal, or executable merge-check job.

## Correction source inspection — 2026-10-10 — Replacement gate pending

Inspected head: `cc834158bfde2e87105bbd2c9620f51815d0c6aa`.
Parent: `e6b67617f2c48a4a6b7974e6fa0f5ed690df71bf`. Base remains `cf3bcaa93faa240b418fa2248884a96581fd82c3`.
Receipt: `msg_plugin-w_1791693171_53d148`. The complete correction changes only the R-52 positive proof and its comments.

SI-2 / F115 closes its reported source path.
Both measured windows bound each client read at 64 bytes. Each step then calls CoreApi::pump and poll_events.
The no-frame control performs 32 steps and requires a peak below FRAME, which is 1 MiB.
The client's frame construction and write occur before the subject window resets its peak.
The subject window performs the same 32 steps and requires a peak of at least FRAME.
The client's read buffers and bounded stream queue cannot satisfy that threshold.
TestkitCore::pump runs Workers::run on the calling thread. The worker receives and buffers the large frame through its ordinary path.
The proof uses harness controls, RouteClient, and CoreApi. It adds no test branch or measurement hook to the worker.
The existing negative proof still excludes allocations from another thread.

I read `evidence/p3-pr3/alloc-f115-reverts.log` completely.
Removing the explicit pump and event poll fails the subject assertion with peak 64: seven passed and one failed.
Restoring the old 1 MiB read bound fails the no-frame control with peak 1048576: seven passed and one failed.
The log supplies both assertion messages. It contains no complete exact-head gate record or final-head reversal header.
These reversals close the reported client-buffer counterexample in source. Gate acceptance remains pending.

The new Prior art section supplies the requested vault notes, old Core comparison, library alternatives, and hand-written-part reasons.
I read both vault notes and the pinned old CountingAllocator and CappedAllocator source.
The old wrappers use System and const thread-local counters. The service allocator uses process-wide charges and enforces a cap.
Source search finds no unsafe_code check in the pinned old Rust sources or manifests.
The supplied reuse and rejection reasons fit this allocator's required observation scope.
One description still needs correction before SI-1 / F116 closes completely.
The first vault paragraph says the transcript asserts a far bound. The pinned transcript instead compares peak_large <= peak_small.
It uses declared lengths 4097 and 0x7FFFFFFF and requires frame_too_large for both. I requested that body-only correction.

`git diff --check` passes. Product code, the unsafe check, exclusions, and conformance lists remain unchanged from the inspected parent.
The implementer schedules the replacement gate after PR B. The replacement gate and same-head package artifact remain pending.
This source inspection completes no round and grants no CLEAN verdict.
I changed no product code and ran no build, test, gate, mutation job, measurement, reversal, or executable merge-check job.

## Published body correction — 2026-10-10

Head remains `cc834158bfde2e87105bbd2c9620f51815d0c6aa`.
Receipt: `msg_plugin-w_1791694987_cfc0ed`.
I verify the published GitHub body and full head after P3 reports that publication now succeeds.
The Prior art section now separates the older far-bound lesson from the current differential transcript assertion.
It names declarations 4097 and 0x7FFFFFFF, frame_too_large in both cases, and `check: le [$peak_large, $peak_small]`.
That wording matches the pinned transcript already read in the source inspection.
SI-1 / F116 LOW closes. SI-2 / F115 remains closed in source at this head.
The package reviewer independently verifies the same body correction in `msg_plugin-w_1791695033_526027`.
Both reported findings now have verified source or documentation corrections.
The replacement exact-head gate, package artifact, and final READY remain pending.
This body inspection completes no formal round and grants no CLEAN verdict.
I run no reviewer execution job.

## Final READY evidence inspection — 2026-10-10 — Package artifact pending

READY head: `cc834158bfde2e87105bbd2c9620f51815d0c6aa`.
Base: `cf3bcaa93faa240b418fa2248884a96581fd82c3`.
Receipt: `msg_plugin-w_1791695059_7e623c`.
The complete source delta after the initial e6b67617 inspection remains the one corrected positive proof: 32 insertions and 11 deletions.
The final source diff check passes. No other source, list, pin, or exclusion changes after that initial inspection.
The only mutation-configuration change from the base remains the reviewed proof names and their comment. No exclusion is added.

Exact-head gate: `~/botster-sessions/gates/botster-core-stage1-p4a-alloc-peak-cc834158-pool-20261010-215500-44897.log`.
I read the exact head and base, all result summaries, conformance reports, positive and negative allocation proof results, and every mutation outcome.
All ten CI steps pass in 364.9 seconds on msa1.
The job exits zero after 524 seconds, with 152 queued seconds and 372 running seconds. The wrapper exits zero after 525 seconds.
Default: 1596 passed, 484 skipped, in 11.521 seconds.
Slow: 408 passed, 1987 skipped, in 91.500 seconds.
Both testkit conformance reports show 206 passed and zero failed.
Strict pending runs 373/394, with 21 real-only IDs and two held IDs.
Testkit minimum is 55/69; real-passing is 35/68; real-accepted is 36/69.
Real-pending remains 83, with zero passing IDs.
The real runner names DP-3 as testkit-proven and does not claim a real allocator measurement.

The removed DP-3 ID passes in 0.206 seconds through the ordinary testkit harness.
The corrected per-thread worker-frame proof passes in 0.008 seconds.
The other-thread negative proof, first-allocation proof, zeroed/realloc proof, controls, and separate no-allocator target all pass.
All seven generalized unsafe-exception proofs pass.
Stage 1 records 46 distinct mutants: 45 caught, one unviable, zero missed, and zero timeouts, in 197.3 seconds.
I read all 46 individual outcomes, including the testkit controls and unsafe-exception decisions. No mutant identity repeats.
Stage 2 skips because stage 1 has no miss. The complete mutation step passes in 199.4 seconds.
No failed, timed-out, or signal-ended test result occurs in the gate.

Both reported findings have verified corrections. The exact-head gate is now verified.
The formal package artifact remains pending, so this inspection completes no integration round and grants no CLEAN verdict.
I run no reviewer execution job.

## Round 1 — 2026-10-10 — CLEAN

Reviewed head: `cc834158bfde2e87105bbd2c9620f51815d0c6aa`.
Base: `cf3bcaa93faa240b418fa2248884a96581fd82c3`.
READY receipt: `msg_plugin-w_1791695059_7e623c`.
Tier: HIGH under BUILD rules 1 and 3. Scope includes the shared testkit and unsafe-exception gate decisions.
Authority: approved plan 23ag, the lead's allocator option A, and pinned R-37/R-52 at contracts `b161db87`.

I read the complete package Round 158 / package round 1 CLEAN artifact `580cc6feabd44c0e572204d21b94fcdc26278f24`.
Its file is `verdicts/p3-worker.md`. Receipt: `msg_plugin-w_1791695302_7b9eae`.
The package artifact names this exact head and base and closes F115 and F116.
The final GitHub read confirms that the PR remains open at the reviewed head, targeting v1.
Origin/v1 remains the reviewed base. The published Prior art section retains the verified differential transcript wording.

This round accepts all twelve own changed paths and the complete corrected positive proof.
It includes the allocator boundary, exact unsafe exception, conformance accounting, source merges, and supplied correction records described above.
SI-1 / F116 LOW closes through the published old-code comparison and corrected transcript description.
SI-2 / F115 MEDIUM closes through the 64-byte control and subject windows, explicit public pumps, and retained red reversals.
The positive window requires the worker's large allocation on the calling thread. The negative proof excludes allocations on another thread.
The allocator library installs no global observer for its consumers. The test binaries install the observer explicitly.
The exact exception check retains the anchor exception and allows only the named PeakAllocator GlobalAlloc implementation in the testkit.
The machine receives no production hook, private helper accessor, or alternative worker path.

Only DP-3 leaves core-pending. Core A20-1 permits its testkit-proven classification; its separate held list obligation remains pending.
Testkit minimum becomes 55/69. Real-passing remains 35/68, and real-accepted becomes 36/69 through that permitted classification.
No real-pending ID, minimum ID, or held ID is removed.
The accepted #227 import remains intact. The earlier mutation count included that stacked source; the final base contains #227.
The correction adds no mutation exclusion. The exact-head gate covers all current own changes and passes every required step.
All 46 mutation outcomes are read: 45 caught, one unviable, zero missed, and zero timeouts. Stage 2 validly skips.
The supplied reversal records retain their stated limits; the final exact-head gate supplies acceptance evidence.

No integration finding remains open within this PR's scope. Later real controls and the production route driver remain separate work.
I change only this verdict file and run no build, test, gate, mutation, measurement, reversal, or executable merge-check job.

VERDICT: CLEAN (0 open)
