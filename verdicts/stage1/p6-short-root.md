# PR #272 — short harness root

## Round 1 — source and correction review, 2026-10-11

Head: `7e899c88bddc985ce8ac9b0fe5b943cff96beed7`.
Base: `010f2a49952f9b77581f304dbaa41006b115d40b`.
Parent: `9d153e3cec74d34a15d3ac10f25d0eb092a88fcb`.
Tree: `a305945fe56674d400775ba527aa781e97019797`.
Risk: HIGH. The change affects the shared test harness.

I read the complete one-file delta at the parent head, its full body, and the harness creation and cleanup contexts.
I read the complete correction delta and current body. GitHub identifies this exact head and base.
The own delta changes only `crates/botster-core-testkit/src/real.rs`: 39 insertions and one deletion.
No pin, dependency, mutation exclusion, pending list, or production code changes.

The harness uses tempfile 3.27.0 to create a unique directory under `/tmp` with prefix `bc-` and mode `0700`.
I read the locked version and its Unix directory creation code. The library applies the mode at directory creation.
The explicit parent removes the inherited TMPDIR length from the worker endpoint path.
Both plain and tapped harnesses use this creation path. TempDir keeps automatic removal on drop.
The existing guard owner precedes the root owner. The change adds no process, wait, deadline, or custom cleanup mechanism.

The test starts no process. It checks the public root parent, mode, address validity, uniqueness, and removal.
The address uses the longest instance identifier and a 16-byte data name. Core checks the same endpoint form.
The test changes no environment variable. SocketAddr validates the platform's limit.

### Package finding and correction

I read both complete package rounds at `68934f1dca2fbde3c287b134926c0001142e8ab5`, `verdicts/p6-short-root.md`.
Round 1 at `6fb4fc419e75e215dcdffbf2f6dd6a092c3fd3f6` reports SR-R1-1 LOW on parent head `9d153e3c`.
The original expected parent used the implementation's SHORT_ROOT constant. A change to `/var/tmp` could preserve that assertion.
I agree with the source deduction. The correction uses literal `/tmp` as the required public result.
All other assertions remain unchanged. SR-R1-1 closes. The package source verdict is CLEAN at this exact head and base.

I read the complete supplied correction log. The positive test passes exactly one selected test at this head, with zero tracked changes.
The supplied reversal changes SHORT_ROOT to `/var/tmp`. The same test fails at line 2306 with observed `/var/tmp` and expected `/tmp`.
The command restores the file. The wrapper exits zero after 82 seconds: 62 queued and 20 running.
The pipeline masks the reversed test's exit status, but the explicit failed result and assertion report establish the failure.
I ran no reversal.

Correction log: `~/botster-sessions/gates/botster-core-stage1-p6-short-root-7e899c88-pool-20261011-081953-17947.log`.
SHA256: `2647bd3e240a1b9995a5252858f5dc4ceaa1460cfef1f6140f679287d5dbeb17`.

### Prior art

I read old Core only at binding pin `72b2e3354ffc291e39f9a5d7eb2f9c5fcbb5e79c`.
Its daemon integration test uses `/tmp` and a short prefix in `short_temp_data_dir` around line 8473.
Its endpoint assertions around line 3955 require paths of at most 103 bytes for macOS.
The body identifies those reused forms. It explains the replacement of manual naming and length checks with library facilities.
The maintained library owns uniqueness, creation permissions, and removal. Current Core retains its configuration refusal for an invalid socket path.

### Supplied parent-head evidence and limits

I read the full Mac log at `9d153e3cec74d34a15d3ac10f25d0eb092a88fcb`.
Its checkout prints that head, tree `aa2ff67430f13fa69f2142cadb57d78e7fd04bbe`, and zero tracked changes.
The job prebuilds three candidate binaries. It sets a 92-byte TMPDIR in the job process.
The root test, A7-1 screen-cap case, and EV-8 positive-deadline case each pass one selected test.
The wrapper exits zero after 89 seconds. This supplies focused native Mac proof for the unchanged implementation.
It supplies no complete Mac conformance run or pending-list removal proof.

Mac log: `~/botster-sessions/gates/botster-core-stage1-p6-short-root-9d153e3c-pool-20261011-081533-3176.log`.
SHA256: `894da364b6c7f455eb0a11667831ef32da182e9f11affdeef7d59726890b2c00`.

I read the Linux gate header, summaries, raw outcomes for both mutants, and the stage-2 baseline result at that parent head.
All ten stages pass in 177.8 seconds. The wrapper exits zero after 320 seconds: 132 queued and 188 running.
Default passes 1693 tests. Slow passes 549 tests. The slow mutation baseline also passes 549 tests.
Stage 1 misses both slow-only mutants. Stage 2 finds both unviable, with zero final missed or timeout mutants.
Both replace an io::Result return with `Ok(Default::default())`. The result types have no Default implementation.
The final result is two unviable mutants and zero catches. This result supplies no viable mutation catch.
The reported minimum remains 56/69 testkit, 46/68 real-passing, and 47/69 real-accepted.
Six of 30 pending-real identifiers pass. This PR removes none, and this review approves no removal or minimum increase.
I do not claim to read every ordinary passing test line in this log.

Linux log: `~/botster-sessions/gates/botster-core-stage1-p6-short-root-9d153e3c-pool-20261011-080956-87643.log`.
SHA256: `2ef3e383d91b704895c155e34152163099bb2be72382884b0ea67a5cb00aa94e`.

The body preserves earlier failed gates as history. I make no independent claim about those unread historical logs.
The completed full gate and focused Mac proof belong to parent head `9d153e3c`.
The lead orders the final gate after both source CLEANs. The final gate and updated final body remain pending at `7e899c88`.
This verdict completes source review with zero open findings. It accepts no future gate or source import.
I ran no build, test, gate, mutant, reversal, measurement, or executable base check.

VERDICT: CLEAN (0 open; source review complete; final exact-head gate pending)

## Round 2 — shared-file import and final gate, 2026-10-11

Head: `61211358870fccaebc064424134f82e1d82c55d6`.
Base: `e4b6cdf54a7453af182b9e3e44fc08b5f58c7b47`.
Parents: `7e899c88bddc985ce8ac9b0fe5b943cff96beed7` and the base.
Tree: `cdc886baf9810003ffeb6b1282b6f2a46ec4c2c1`.
Package source CLEAN: `f78a09735c318f284af55b43d81eda9bf3e32f66`, `verdicts/p6-short-root.md`, Round 3. I read the complete round.

I read the full failed base check, shared-file import, current own delta, and every body change.
Both ancestry checks and condition 1 pass. Conditions 2 and 3 fail on shared real.rs and its changed index line.
Both pending-file checks pass. The failed result remains failed; this round performs the required delta review.
The import adds only the reviewed route_reports argument, implementation, capability, dispatch arm, and control assertions.
It changes no short-root helper, construction call, ownership, or root assertion. The imported tests use the same make path.
Both complete own patches match after removal of index lines and normalization of hunk positions.
The static merge produces the exact new tree. Cargo.lock, mutation configuration, and both pending files equal the new base.
No finding remains. SR-R1-1 stays closed.

The base is the #271 merge. Its exact parents are `010f2a49952f9b77581f304dbaa41006b115d40b` and `eda6efc927d3b53471c7693e6e73f802301e1dd3`.
Its tree `4b07c85a40098941ad1cfdb597e3b4c0f985c925` equals the accepted #271 tree.

I read the final exact-head Linux gate header, summaries, both stage-1 raw outcomes, and both stage-2 raw outcomes.
All ten stages pass in 206.1 seconds. Default passes 1700 tests; slow passes 549 tests.
Stage 1 misses both slow-only mutants. The stage-2 baseline passes 549 tests, and stage 2 finds both mutants unviable.
Final result: two unviable, zero caught, zero missed, and zero timeout mutants. This supplies no viable mutation catch.
No FLAKY record appears. The wrapper exits zero after 214 seconds; jobq exits zero after 213 seconds.
The final body identifies this exact head and gate. It records the failed base check and preserves earlier head evidence as history.
The minimum remains 46/68 real-passing and 47/69 real-accepted. Four of 30 pending-real identifiers pass; none leaves its list.

Gate: `~/botster-sessions/gates/botster-core-stage1-p6-short-root-61211358-pool-20261011-082558-36579.log`.
SHA256: `5a663bc70b7df5c6951728aeda3fa5a590c9e1d9dde77b516079e7e0677e05c5`.
Base check: `~/botster-sessions/gates/botster-core-stage1-p6-short-root-61211358-pool-20261011-082534-35425.log`.
The unchanged Mac implementation and assertions retain the parent-head proof limits from Round 1.
GitHub identifies this exact head and base. I accept no future source import or pending removal.
I ran no build, test, gate, mutant, reversal, measurement, or executable base check.

VERDICT: CLEAN (0 open; source, final body, and supplied exact-head gate accepted)

Package completion receipt: I read the complete Round 4 at `d7d979a43431cd0b332c498dc89c4de1dc0b54fd`.
It accepts the same final head, body, and gate with the same proof limits. Both completed reviews are CLEAN.

## Round 3 — oracle_resume import and final gate, 2026-10-11

Head: `7e07b775fd420debc664792f6416a5997e5228de`.
Base: `d582853da895688710ab97f8f1a0858599e8ba18`.
Parents: reviewed `61211358870fccaebc064424134f82e1d82c55d6` and the named base.
Tree: `999bd68f85420be82f0bc36e9a27cdfac80f7b10`.
Lead receipt: `msg_plugin-w_1791734065_873cc7`.
Package source verdict: `833eddc2754c68fced81f96c14b1a2e1080ea8f5`, Round 5, read in full.
Package completion receipt: `msg_plugin-w_1791734118_cd3f02`, verdict `4ad792c120e320a8ad0b7ed952c1e9efe94263c4`.

I read the complete current own delta and complete overlapping real.rs import.
The nine other imported paths match the exact base blobs from the reviewed #269 head.
The base merge has parents e4b6cdf5 and c2dd1fbf, with #269's reviewed tree `91087a9b`.
The static merge tree matches this round's tree exactly.
Both complete own patches match after removal of index lines and hunk-position lines.
My comparison omits the final newline: 3247 bytes, SHA256 `db2e2c25119ea2d478a57ee88fca62a60ab919df86d4c562cdf6c6acb74c4bb9`.
The package comparison retains that newline and reports 3248 bytes. These normalizations give the same equality result.
Cargo.lock, .cargo/mutants.toml, and both pending files match the base exactly.

The import puts CaptureLayer inside Crashable and retains a weak record handle in the harness.
The layer owns the driver. Dropping the crash slot drops both. The program mark view uses the existing shared state.
The plain open path remains unchanged. The short-root constructor runs before either open path.
The import adds the oracle_resume dispatch and capability, four test helper visibilities, and a weak test field.
It changes no SHORT_ROOT, short_root, make correction, cleanup field order, or root assertion.
The root test opens no Core and starts no process. SR-R1-1 remains closed. Zero findings remain.

Base-check log: `~/botster-sessions/gates/botster-core-stage1-p6-short-root-7e07b775-pool-20261011-084527-12903.log`.
SHA256: `1ab634d761d528d2474ddaa94895c577c4dbaa9b08df5f1cb05b8e7a87d2da53`.
I read the complete log across bounded reads. Ancestry, condition 1, and both condition-4 lists pass.
Condition 2 fails on shared real.rs. Condition 3 fails at the changed index line.
The job exits one after 152 seconds, with 137 queued and 15 running. The wrapper exits one after 153 seconds.
This failed check requires this source review; it supplies no automatic transfer.

Final gate: `~/botster-sessions/gates/botster-core-stage1-p6-short-root-7e07b775-pool-20261011-084813-17823.log`.
SHA256: `62bd8683e835616aa819d927d934b0c86de4af913ce6837aeae6ec4efbc869e1`.
I read the exact-head and base headers, summaries, root test records, and all four mutation outcomes.
All ten steps pass in 194.5 seconds. Default: 1712 PASS, 481 skipped, 13.049 seconds.
Slow: 551 PASS, 2011 skipped, 42.165 seconds. The root test passes.
The slow mutation baseline passes 551 tests in 41.112 seconds; its build and run take 51.9 seconds.
Stage 1 misses both slow-only mutants in 9.0 seconds. Stage 2 finds both unviable in 2.1 seconds.
Both replace the short_root or make return with Ok(Default::default()). TempDir and RealCoreHarness have no Default implementation.
Final result: zero caught, missed, or timeout mutants; two unviable. This supplies no viable mutation catch.
No FLAKY record appears. The job and wrapper exit zero after 323 seconds, with 121 queued and 202 running.
The real minimum stays 46/68 and 47/69. Five of 30 pending-real identifiers pass; all remain pending under the .38 pin rule.
The prior focused Mac proof and explicit reversal retain their reviewed heads and scope.

I read every final body change against the previously read complete body. GitHub names this exact head and base.
The body identifies the final gate, failed base check, source import, and retained pending identifiers.
This round completes the source, body, and supplied final-gate review. It approves no future import or pending removal.
I ran no execution job.

VERDICT: CLEAN (0 open) at 7e07b775fd420debc664792f6416a5997e5228de
