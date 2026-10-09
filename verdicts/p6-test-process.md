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
