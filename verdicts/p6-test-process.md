# P6: shared test-process crate

## Round 1 — PR #171

Implementation head: `dc7fca5a1bfa207b4a103b457c4981baaa25e975`.
Integration base: `59ce126885e04a3b3f22d97e89337bfdbf055949`.
Branch: `stage1/p6-test-process`.
PR: <https://github.com/trybotster/botster-core/pull/171>.
Plan: `stage1-plan.71a623ef.md`, sha256 `71a623ef93f487754e400dc186429216357e39fe251933675cc7f851843d519d`.

The review covers all 19 changed files. It includes ownership, reads, platform adapters, the anchor, tests, prebuild changes, and mutation exclusions.
The lead permits one PR for this crate. The syntax check, run wrapper, and migrations remain separate work.
The reviewer changed no product code. The reviewer ran no builds, tests, mutation jobs, or gates for this round.

### TP1 — HIGH: blocking waits escape the deadline

Locations: `child.rs:139-157`, `child.rs:191-203`, `rounds.rs:146-154`, and `bin/botster-test-anchor.rs:86-91`.
All paths are under `crates/botster-test-process/src/`.
The test helper `tests/slow_process.rs:44-47` has the same wait shape.

`OwnedChild` bounds the exit event, then reads status with blocking `waitid(EXITED | NOWAIT)` or reaps with `Child::wait`.
Neither call receives the deadline. The wrapper also calls `intermediate.wait()` after the event.
`end_group` calls `reserve.wait()` on success and failure. A listing or kill error can occur before any kill or exit observation.
The reserve can thus remain live or stopped when the blocking wait starts.

An exit event does not prove that status is available on macOS. The source acknowledges this distinction in `child.rs`.
XNU sends `NOTE_EXIT` before it sets `SZOMB` and signals the parent with `SIGCHLD`.
This ordering supports the finding. It does not establish a time bound for the later wait.
See [XNU process exit](https://raw.githubusercontent.com/apple-oss-distributions/xnu/main/bsd/kern/kern_exit.c).

Keep the deadline through completed exit, status observation, and the exact-pid reap.
Kill the owned reserve on an error path before its bounded completion check.
Keep group reservation until group cleanup ends. Report failure at the existing bound.
Prove the behavior when an exit event occurs but status is not yet available.
The integration review's R1 concerns this same reserve wait. It is a duplicate of TP1.

### TP2 — MEDIUM: failed identity checks permit a signal

Locations: `bin/botster-test-anchor.rs:135-153`, `platform/linux.rs:9-14`, and `platform/macos.rs:10-14`.

Both start-time adapters convert every read error to `None`.
`verify` treats `None` as proof that the leader is gone or its pid was reused.
Its `getpgid` match also accepts every error. The comment justifies only `ESRCH`.
An unavailable identity or group can thus bypass the required refusal after a group move.

Distinguish a verified end or reused identity from a failed read.
Refuse cleanup when the leader's identity or membership cannot be verified.
Keep the permitted cleanup of an old, reserved group after its verified leader ends.
Add observable refusal proof for the identity and membership error paths.

### TP3 — MEDIUM: helper stages keep extra inherited descriptors

Locations: `bin/botster-test-anchor.rs:67-79`, `bin/botster-test-anchor.rs:125-130`, and `bin/botster-test-anchor.rs:178-200`.

The wrapper replaces descriptors 0 through 2 for the intermediate.
The intermediate and anchor do not close extra descriptors that lack close-on-exec protection.
An intentionally inherited production writer can thus remain open in the anchor after production exits.
That writer can prevent a pipe or FIFO from reaching EOF. The handoff requires descriptor isolation.

Close extra descriptors in helper stages. Preserve the descriptors that the real program must receive in the wrapper.
Prove public EOF behavior with an intentionally inherited writer while the anchor remains live.
The implementer requested a lead ruling about descriptor closure under the ban on unsafe code.
This review grants no exception to that ban.

### TP4 — HIGH: startup errors abandon the intermediate

Location: `bin/botster-test-anchor.rs:67-110`.

The wrapper owns a raw `Child` for the intermediate.
A timeout or error from `await_end` returns without killing or reaping that child.
Dropping `Child` does neither. If the intermediate stalls before it starts the anchor, guard EOF has no anchor to end the group.
The guard reports the wrapper error but cannot clean up an unreported group safely.

Give the intermediate ownership and bounded cleanup on every error path.
Reap only that owned child, by its exact pid.
Prove startup failure and test death before anchor registration without an orphan.
Use the existing cleanup bound. TP1 also applies to this child's reap.

### TP5 — MEDIUM: real panic cleanup has no proof

Locations: `tests/slow_process.rs:59-65`, `tests/slow_process.rs:197-224`, and `tests/slow_process.rs:278-319`.

The slow tests prove normal guard drop and killed-test cleanup.
`drop_failure` catches a panic that cleanup raises during a normal drop.
It does not drop a real guard while a test panic unwinds the stack.
The binding anchor ruling requires panic cleanup proof on Linux and macOS.

Add that proof for the real guard.
Assert that the production group ends. Assert that production can still reap its exact child.
Supply Linux and macOS evidence for the new path.

### TP6 — HIGH: membership is not checked after TERM grace

Location: `bin/botster-test-anchor.rs:201-208`.

The anchor verifies membership before `terminate`. It does not verify membership after the configured TERM grace.
A live nonleader can move to its own group after TERM.
The anchor then kills the old group and reports success while the production process remains live in its new group.
The binding ruling requires verification after the grace and before KILL.

Verify the recorded identity and membership again after the grace.
Prove a group move triggered by TERM through the public guard path.
Assert refusal. Assert that the anchor sends no KILL to an unverified group.

### Review evidence and boundaries

The Prior-art note records a reuse, rejection, or scope decision for every item required by the brief.
It covers `process-wrap`, `wait-timeout`, nextest, the census, existing guards, `OwnedGroup`, and the old launch and supervisor code.
It gives a reason for the anchor, rounds, and FIFO code. Its blocking-wait change remains subject to TP1.
No production crate gains a dependency on this crate. The prebuild includes the anchor in the sha256 manifest.
The reserve uses an exact-pid wait. The guard does not reap production children.
The test that retains production's reap asserts the child's signal and pipe EOF.
No source sleep or shell sleep loop was found. The rounds retain the existing cleanup limit.

The supplied full Linux gate for this head passes 858 default tests and 228 slow tests.
It reports 103 caught mutants and 30 unviable mutants, with no misses or timeouts.
Log: `gates/botster-core-stage1-p6-test-process-dc7fca5a-pool-20261009-001409-97947.log`.

The separate in-diff run with `NEXTEST_PROFILE=slow` reports 101 caught, 30 unviable, 15 off-platform misses, and two timeouts.
Log: `gates/botster-core-stage1-p6-test-process-dc7fca5a-pool-20261009-001802-4648.log`.
The two mutants affect `Deadline::expired` and `Bounded::line`.
A focused run changes the nextest policy to `--max-fail 1:immediate`.
Its log shows an actual assertion failure before cancellation for each mutant. It reports both caught.
Log: `gates/botster-core-stage1-p6-test-process-dc7fca5a-pool-20261009-002301-9071.log`.
These assertions establish behavioral detection. They do not prove that the current gate enforces the planned mutation policy.
PR B still owns that policy and its red-on-revert proof.

The focused Mac run at `2d1c53a7947b06a73ff2bd067efce19eba7dda4e` passes 51 tests.
It reports 13 caught mutants and two unviable mutants, with no misses or timeouts.
Log: `gates/botster-core-stage1-p6-test-process-2d1c53a7-pool-20261008-232847-71035.log`.
The crate's source is unchanged between that head and this head. This head adds the prebuilt-anchor test.
The Mac evidence does not close TP5 or prove the required fixes.

The reviewer sent TP1 through TP6 directly to the implementer.
The implementer acknowledged TP1 through TP5 and proposed bounded status observation for TP1.
No replacement implementation head has a package verdict yet.

VERDICT: NOT CLEAN

## Round 2 — PR #171

Implementation head: `5d0d89c7d443146418117e5db991d3ea81b61c36`.
Previous reviewed head: `dc7fca5a1bfa207b4a103b457c4981baaa25e975`.
Integration merge base: `59ce126885e04a3b3f22d97e89337bfdbf055949`.
The supplied gates use `origin/v1` at `a0f78fe4c4e4faff1fee926e072ceb5e94ba8649` as their base ref.
The reviewer inspected all 14 files in the delta and the supplied logs.
The reviewer ran no build, test, mutation job, or gate. The reviewer changed no product code.

### Closed findings

- **TP1:** `await_status` uses `waitid` with `NOWAIT | NOHANG`, a deadline check, and a real event.
  macOS registers the SIGCHLD filter before its first status check. Linux waits on a pidfd.
  Status observation does not reap. `reap_within` uses `try_wait` for the exact child.
  The reserve receives KILL before its bounded reap, including after a failed cleanup round.
  The fake event sequence proves delayed status and deadline behavior. The slow test retains the child's status for its owner's reap.
- **TP2:** the adapters return read errors separately from a verified end.
  The public `verdict` function refuses identity errors and membership errors other than ESRCH.
  Its tests assert refusal and the permitted ended-leader outcome.
- **TP3:** both helper stages first call `close_inherited`.
  The function retains descriptors 0 through 2 and its directory descriptor. It closes the directory descriptor last.
  The EOF test passes while the anchor remains live. The wrapper retains the real program's inherited descriptors.
  The lead permits this one unsafe function. TP7 below concerns the new check that must enforce that permission.
- **TP4:** `start_anchor` owns the intermediate through `OwnedChild`.
  Timeout and error paths retain its bounded Drop cleanup. The stalled-start test asserts pipe EOF.
  The killed-parent test starts a wrapped program without accepting or awaiting an anchor, then asserts pipe EOF after parent death.
- **TP5:** the new test panics while the real guard is in scope.
  It asserts production's exact-child KILL status and pipe EOF after the stack unwinds.
  The Linux and Mac logs select and pass that test.
- **TP6:** the anchor verifies the leader after TERM grace and checks the members that it listed before TERM.
  A moved member causes refusal before KILL. The slow test triggers a move with TERM.
  It asserts the refusal and the old group's leader exit code 0. The Linux and Mac logs pass that test.

### TP7 — HIGH: the unsafe-code check misses valid syntax

Locations: `xtask/src/unsafe_code.rs:55-95`.

`other_attributes` inspects each trimmed source line separately.
It requires that one line both start an attribute and contain `unsafe_code`.
It therefore misses this valid attribute on a second function inside the crate:

```rust
#[allow(
    unsafe_code
)]
fn another() {
    unsafe { libc::close(3); }
}
```

The crate now uses `deny`, so this attribute permits the second unsafe function.
The scanner reports no problem for it. The lead's permission requires the check to enforce one function.
The current red-on-revert tests use single-line attributes and do not cover this failure.

`other_manifests` also searches raw text instead of the parsed lint table.
An escaped quoted TOML key can define the lint without the literal text that the scanner searches:

```toml
[lints.rust]
"unsafe_\u0063ode" = "allow"
```

Parse the attributes and the items that own them. Check nested items and `cfg_attr`.
Parse manifest lint keys before checking other manifests.
Add red-on-revert fixtures for multiline `allow` and `expect`, a second function, and an escaped manifest key.
Keep the lead's one-function permission exact.

### TP8 — MEDIUM: the new exclusion reason omits poll errors

Location: `.cargo/mutants.toml`, the new `platform/macos.rs` `await_status` error-arm exclusion.

The reason states that kqueue reports an error event only for a failed registration.
It states that `watch` already returns that error, so no registered signal filter can reach the arm.
The crate's `EventData::Error` also represents a later failure of the `kevent` call.
In kqueue 1.2.1, `event.rs:127` maps a return of -1 to `Event::from_error`.
`event.rs:225` constructs an event whose data is `EventData::Error`.

Apple documents EINTR when a signal interrupts `kevent` before the timeout or an event.
That failure can occur after successful registration.
See [Apple's kevent manual](https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man2/kevent.2.html).
The code handles this error correctly. The exclusion's unreachable-arm argument is incorrect.

Correct the reason. Supply behavioral proof or an accurate, scoped reason for excluding this mutant.
Do not classify later poll errors as registration errors.

### Evidence and scope

The full Linux gate on this head passes 873 default tests and 236 slow tests.
It reports 153 caught mutants and 30 unviable mutants, with no misses or timeouts.
Log: `gates/botster-core-stage1-p6-test-process-5d0d89c7-pool-20261009-013207-11707.log`.

The raw slow-profile in-diff run reports 151 caught, 30 unviable, 18 off-platform misses, and two timeouts.
Log: `gates/botster-core-stage1-p6-test-process-5d0d89c7-pool-20261009-013552-15018.log`.
The focused run with `--max-fail 1:immediate` catches both timed-out mutants through assertion failures in 2 ms.
Log: `gates/botster-core-stage1-p6-test-process-5d0d89c7-pool-20261009-014137-18902.log`.
The lead's 2026-10-09 ruling permits this fail-fast policy. PR B still owns its gate integration and hang fixture.

The focused Mac run selects 67 tests and passes all 67. It skips the prebuilt-anchor test, which requires xtask prebuild.
It reports 16 caught mutants and two unviable mutants, with no misses or timeouts.
Log: `gates/botster-core-stage1-p6-test-process-5d0d89c7-pool-20261009-014149-19171.log`.
The Mac and Linux logs pass the new EOF, stalled-start, killed-parent, panic, and post-grace refusal tests.
The tests assert process status, refusal, and EOF. They preserve production's exclusive reap.

The Prior-art note describes the new bounded status wait and the permitted descriptor closure.
The existing per-OS exclusions remain temporary until PR B derives them from `cfg`.
The Linux run wrapper and migrations remain outside this verdict.
TP7 and TP8 went directly to the implementer. No lead decision is required for this NOT CLEAN round.

VERDICT: NOT CLEAN

## Round 3 — 2026-10-09

Reviewed head: `414b0ab1e6e3d3ad55da56c87fe2f12f4fe18b32`.
Previous reviewed head: `5d0d89c7d443146418117e5db991d3ea81b61c36`.
Integration merge base: `59ce126885e04a3b3f22d97e89337bfdbf055949`.
The supplied gate uses origin/v1 `a0f78fe4c4e4faff1fee926e072ceb5e94ba8649` as its base ref.
This round reviews the delta from round 2. TP1 through TP6 remain closed.

### Closed findings

TP7 closes. `unsafe_exception.rs` reads Rust tokens and parses TOML.
The source check permits one token position in the top-level `#[allow(unsafe_code)]` of `fn close_inherited` in the anchor binary.
It finds multiline attributes, `expect`, `cfg_attr`, nested items, macro bodies, and raw identifiers.
The manifest check finds escaped keys and lint names in Cargo configuration flags.
The fixtures assert the check's result for each reported bypass and for the permitted exception.
The exact-head gate runs these fixtures and reports no finding in the real tree.

TP8 closes. The inaccurate exclusion is removed.
`polled` returns distinct timeout, event, and interruption results. It preserves other errors.
The Mac default-tier test asserts all four results. The Mac mutation run covers this decision.
The `await_status` caller checks status and expiry again after an interruption.
The `await_end` caller introduces TP9 below.

### TP9 — MEDIUM: the Mac interruption loop can retry after its deadline

Location: `crates/botster-test-process/src/platform/macos.rs:46-54`.

`await_end` retries each `Polled::Interrupted` result without an expiry decision.
After the deadline, `deadline.remaining()` supplies zero. A later interruption still causes another iteration.
Thus repeated interruptions can keep the wait active after its named bound.
The existing `polled` test checks one result. It does not prove that the retry loop stops at expiry.

A zero timeout does not prove that EINTR is impossible on this path.
XNU's legacy `kevent` computes an absolute deadline and calls `assert_wait_deadline` from `kqueue_scan`.
Its interrupted continuation returns EINTR.
See [XNU's event implementation](https://raw.githubusercontent.com/apple-oss-distributions/xnu/main/bsd/kern/kern_event.c).
`waitq_assert_wait64_locked` calls `thread_mark_wait_locked` before it arms the deadline timer.
It arms that timer only for `THREAD_WAITING`.
See [XNU's wait implementation](https://raw.githubusercontent.com/apple-oss-distributions/xnu/main/osfmk/kern/waitq.c).
An already aborted wait returns `THREAD_INTERRUPTED`.
See [XNU's scheduler implementation](https://raw.githubusercontent.com/apple-oss-distributions/xnu/main/osfmk/kern/sched_prim.c).
These paths support the repeated-interruption case; the reviewer did not run a signal-stress test.

Add an explicit expiry exit before an interruption retry.
Preserve an already available exit event if the caller requires that behavior.
Add a decision test with repeated interruptions and expiry. The test must prove that no later poll starts.

### Evidence and scope

The supplied full Linux gate passes all ten jobs: 874 default tests and 236 slow tests.
It reports 183 caught mutants and 33 unviable mutants, with no misses or timeouts.
Log: `gates/botster-core-stage1-p6-test-process-414b0ab1-pool-20261009-030617-17472.log`.

The Mac focused run passes 69 tests and skips the prebuilt-anchor test, which the Linux full gate runs.
It reports 19 caught mutants and three unviable mutants, with no misses or timeouts.
Log: `gates/botster-core-stage1-p6-test-process-414b0ab1-pool-20261009-031627-28957.log`.

The Linux slow-tier run without configuration tests 234 mutants: 187 caught, 35 unviable, and 12 misses with written reasons.
It reports no timeout. Both `OwnedChild::id -> 0` and `-> 1` are caught.
Log: `gates/botster-core-stage1-p6-test-process-5ae04611-pool-20261009-025755-11248.log`.
Only mutation-reason text changes between that head and the reviewed head.
The integration reviewer closes E1 in verdict commit `f91ad7b5ab0bfe7184a34e919405a0404becb98e`.

The new signal-target check refuses group 1 on all three group-signal paths.
Its decision test sends no signal. The shared crate still preserves production's exclusive reap.
The group-end test gains an independent `CLEANUP` bound for its result thread.
The Prior-art table keeps a decision and reason for every item. No migration is included in this delta.
PR B still owns the mutation profile and hang fixture. PR A2 still owns the Linux run wrapper.

TP9 went directly to the implementer. No lead decision is required.
The reviewer ran no gate, build, test, or mutation job. The reviewer changed no product code.

VERDICT: NOT CLEAN
