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
