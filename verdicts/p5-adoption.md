# P5 adoption and restart review

## PR #162 — Round 1

- PR: https://github.com/trybotster/botster-core/pull/162
- Exact head: `b5c4bffbcdf2bc1843f944eafbf315a9b9b200cb`.
- Base: `144b0234fb632bcbb5176b17c2fe55f3239405df`.
- Scope: audit A10 / issue #152, in `crates/botster-core/tests/slow_real_core.rs`.
- This verdict covers P5's package scope. This PR changes one test file and needs no cross-package review.
- The reviewer ran no builds, tests, mutation jobs, or gates.

The PID probe and the false claim that the guard reaps the worker are removed. Production still owns worker reaping.
The test now observes FIFO EOF, as audit A10 permits. Two findings prevent acceptance of the complete replacement proof.

### P5-F1 — MEDIUM — The FIFO does not observe every child

**Evidence:** `slow_real_core.rs:315-338` claims that every process of the script holds the FIFO write end.
The body uses `common::WAIT_WHILE_THE_PARENT_LIVES` from `tests/common/mod.rs:15`.
That loop starts `/bin/sleep 1 >/dev/null 2>&1`. The child redirects its standard output away from the FIFO.

FIFO EOF therefore proves that the worker shell closed its write end. It does not prove that the `sleep` child ended.
If cleanup ends only the shell, EOF can arrive while the child still runs.
The comment, assertion message, and PR description claim a stronger result than the test observes.
BUILD.md testing rule 10 requires cleanup of the process group. The user also requires the guard to own that group.

**Required change:** Keep a FIFO descriptor open in every persistent child used by this test.
Use a dedicated inherited descriptor if a child redirects its standard output.
Alternatively, use a fixture whose persistent child keeps the observed standard output.
Keep the child blocked without a busy loop. Keep production as the sole worker reaper.
Update the comment and PR description to match the observation.

**Closure evidence:** The source must show that the persistent child holds the observed descriptor until exit.
Supply one focused result for the revised test. Do not use repeated runs as proof that a race is fixed.

Status: OPEN.

### P5-F2 — MEDIUM — The cleanup deadline does not bound guard cleanup

**Evidence:** `slow_real_core.rs:360` calls `drop(worker)` before the marked deadline at lines 361-365.
`ScriptWorker` owns `GroupGuard`. Its `Drop` calls `registration.join()` and `anchor.wait()` synchronously
in `crates/botster-core-sys/tests/common/process_guard.rs:80-92`.
Neither wait has a deadline. If guard cleanup blocks, the test never reaches `recv_timeout`.

The replacement test therefore still contains an unbounded cleanup wait.
Its deadline comment says a failed guard causes a test failure instead of a hang, but that statement does not cover this path.
BUILD.md testing rule 5 and the user's real-process rules require bounded waits.

**Required change:** Bound the test's complete cleanup operation, including guard destruction.
For example, move the owned guard cleanup to a helper thread and observe its completion through a marked deadline.
Do not wait for that thread without a deadline. Do not let the test reap the production worker.
Keep the existing group ownership and cleanup on panic.

**Closure evidence:** The source must show that the test reaches a bounded wait before any blocking guard cleanup.
Supply one focused result for the revised test. A shared guard change also needs the applicable package review.

Status: OPEN.

### Evidence and scope limits

The reviewer inspected the implementer's Linux log:
`~/botster-sessions/gates/botster-core-stage1-p5-a10-b5c4bffb-linux-20261004-202957-96965.log`.
The header names the exact head and base above. Clippy completed successfully.
The log reports 20 selected passes and a final run with 15 passed and zero skipped. The command exited 0.
These results establish focused execution. They do not establish either missing cleanup guarantee.
The repeated runs do not replace a structural race proof under BUILD.md testing rule 9.

The removed LC-12 PID check could not distinguish a live worker from a zombie.
The PR explicitly leaves `conf::lc_12_drop_leaves_workers_running` for P5 proper.
This verdict does not establish LC-12 conformance or closure of the other P5 audit findings.

VERDICT: NOT CLEAN (2 open)

## PR #162 — Round 2

- Exact head: `1a96eee7adc240a4ed90835dfc967e2efdd8e6b6`.
- Base: `144b0234fb632bcbb5176b17c2fe55f3239405df`.
- Delta: `b5c4bffbcdf2bc1843f944eafbf315a9b9b200cb..1a96eee7adc240a4ed90835dfc967e2efdd8e6b6`.
- Scope remains audit A10 / issue #152. Only `slow_real_core.rs` changes.
- The reviewer ran no builds, tests, mutation jobs, or gates.

P5-F1's source defect is closed. The shell now waits for `/bin/cat` on a FIFO that has no writer.
The child keeps the observed FIFO as standard output. Neither process redirects that output or runs a busy loop.
FIFO EOF establishes that all write descriptors closed, without depending on production reaping.

P5-F2 is CLOSED. The test moves `worker` to a cleanup thread before guard destruction.
The test observes cleanup completion through a marked 10-second deadline, then observes FIFO EOF through another marked deadline.
The shared guard and production worker reaper are unchanged.

The reviewer inspected the exact-head Linux log:
`~/botster-sessions/gates/botster-core-stage1-p5-a10-1a96eee7-linux-20261004-205647-55445.log`.
Its command runs format checks, Clippy, and one `slow_real_core` run.
The log reports 15 passed, zero skipped, and exit 0. The revised cleanup test passed.
This evidence is a focused run. The full merge gate remains subject to the lead's FULL HOLD.

### P5-F1 — Remaining documentation requirement — LOW

The source defect is closed, but the Round 1 documentation requirement remains open.
The reviewer read the full PR body with `gh pr view 162` on this head.
The body still states that the script uses a wait loop that ends when the test process is gone.
The revised script uses a blocked `cat` child instead. The evidence section names only `b5c4bff` and its 20 repeated runs.

**Required change:** Update the PR body to describe the blocked child and the bounded guard cleanup.
Name the exact revised head and its single focused run. Preserve the LC-12 scope limit and Prior art note.
No source commit or new test run is required for this documentation correction.

Status: OPEN at LOW severity. P5-F2 is CLOSED. No other finding is open for this PR's scope.

VERDICT: NOT CLEAN (1 open)

## PR #162 — Round 3

- Exact head: `1a96eee7adc240a4ed90835dfc967e2efdd8e6b6`.
- Base: `144b0234fb632bcbb5176b17c2fe55f3239405df`.
- Change since Round 2: PR description only. The source head is unchanged.
- Scope: audit A10 / issue #152. This CLEAN covers P5's package scope for this PR only.

P5-F1 is CLOSED. The reviewer read the updated full PR body with `gh pr view 162`.
The body now describes the blocked `cat` child, the inherited FIFO output, and the cleanup thread with a marked deadline.
It names the exact reviewed head and the single focused run recorded in Round 2.
The Prior art note and LC-12 scope limit remain present.

P5-F2 remains CLOSED. Audit A10 is CLOSED for this PR's scope.
No finding remains open, including LOW findings. The source and execution evidence from Round 2 remain valid on this unchanged head.
The reviewer ran no builds, tests, mutation jobs, or gates.

This CLEAN does not establish the full merge gate, LC-12 conformance, closure of other audit findings, or completion of P5.
The implementer must respect the lead's FULL HOLD before starting the merge gate.
Any later source commit requires a delta review on its exact head.

VERDICT: CLEAN

## PR #162 — Round 4

- Exact head: `913594cb5aa2afe858aa0ed3f0eaa5b357a373d2`.
- Base: `144b0234fb632bcbb5176b17c2fe55f3239405df`.
- Delta: `1a96eee7adc240a4ed90835dfc967e2efdd8e6b6..913594cb5aa2afe858aa0ed3f0eaa5b357a373d2`.
- Scope now includes the shared guard failure that the lead assigned to this PR after the Mac gate failed.
- The shared guard compiles into worker tests. The integration reviewer must review this delta.
- The reviewer ran no builds, tests, mutation jobs, or gates.

Round 3 CLEAN applies only to its recorded head and scope. It does not accept this new delta.
P5-F1 and P5-F2 remain closed for the original A10 test replacement.
The new scope changes the guard's cleanup premise, so the reviewer checked the shared callers under `orchestrate-delivery`.

### P5-F3 — HIGH — The new guard precondition cannot cover panic or test-process death

**Evidence:** `crates/botster-core-sys/tests/common/process_guard.rs:13-16` adds a precondition for every guard user.
The group must stop creating children before cleanup begins. A fixture must report final-child readiness before the test kills it.
The anchor still performs one group kill and exits at lines 114-117.

The same module retains two callers that do not satisfy this precondition:

- `a_panic_before_ready_ends_the_child` starts `while :; do /bin/sleep 1; done` at line 260.
  It then panics without waiting for readiness at lines 267-270.
- `an_early_exit_keeps_the_group_owned_until_cleanup` starts a descendant with that same repeated-fork loop at line 286.
  It drops the guard while the loop can still create another child at line 302.

`crates/botster-core/tests/common/mod.rs:15` and `crates/botster-core-sys/tests/slow_process.rs`
also retain loops that repeatedly start `sleep` children.
Test-process death can occur before any readiness event, regardless of the normal test sequence.

Under the PR's stated macOS failure mechanism, these paths can still miss a child during the group kill.
Moving child creation before readiness in two fixtures does not establish the shared cleanup guarantee.
BUILD.md testing rule 10 requires cleanup on every exit path, including panic.
The user's real-process rules require the guard to own and kill the group without taking production reaping.
A new caller precondition cannot narrow those requirements.

**Required change:** Preserve cleanup on panic and test-process death during startup.
Close the reported failure through fixture ownership and synchronization that also cover those paths.
Check every shared caller affected by the chosen rule. Remove any precondition that those callers cannot meet.
Keep production as the sole worker reaper. Do not add a repeated-kill polling loop or a timeout increase.

**Closure evidence:** Supply a deterministic regression for cleanup during the relevant child-creation handoff.
The regression must observe process cleanup through a real event and a marked deadline.
Record a failing baseline at the intended cleanup assertion and a passing revised result.
Checking one child's process group before the kill is setup evidence; it does not cover death before readiness.

Status: OPEN.

### P5-F4 — MEDIUM — The changed guard self-test still waits without deadlines

**Evidence:** `parent_dies_before_fifo_reader` reads the readiness line directly at `process_guard.rs:196`.
No deadline bounds `reader.read_line`. A fixture that fails before readiness can leave the test blocked there.
The EOF deadline at line 215 runs only after that read and parent destruction.

Parent destruction also calls `child.wait()` synchronously at lines 147-151.
The test calls `drop(parent)` at line 214 before its bounded EOF observation.
The revised self-test therefore retains waits that its EOF deadline does not bound.
This is the same wait-coverage error that P5-F2 closed in the A10 test.
BUILD.md testing rule 5 and the user's real-process rules require bounded test waits.

**Required change:** Put a marked deadline around the readiness read and the test-owned parent cleanup.
Keep the EOF observation. Do not increase its timeout. Reap only the test-owned parent and anchor.

**Closure evidence:** The source must reach a bounded wait before each blocking operation.
Supply one focused result for the revised self-test. A failed readiness path must produce a bounded test failure.

Status: OPEN.

### Evidence and scope limits

The reviewer inspected the failed Mac gate log on the previous head:
`~/botster-sessions/gates/botster-core-stage1-p5-a10-1a96eee7-mac-20261004-212801-2845.log`.
Its slow tier reports 83 passed and one failed.
`botster-core-sys::slow_process process_guard::parent_dies_before_fifo_reader` failed at the pipe EOF deadline after 10.050 seconds.
The gate exited 1. That log proves the failure; it does not establish the implementer's kernel-level explanation.
The implementer reports an orphaned `tee` observation from a separate diagnostic. The reviewer has requested its raw evidence.

No execution evidence for `913594cb` was supplied with this request.
The lead's gate-evidence rule permits a gate after all source findings close. P5-F3 and P5-F4 are source findings.
The integration reviewer was notified with the exact head and shared scope.

VERDICT: NOT CLEAN (2 open)

## PR #162 — Round 5

- Exact head: `a93a13cb34d295984ca9d1f8767248b719c8a8ee`.
- Base: `144b0234fb632bcbb5176b17c2fe55f3239405df`.
- Scope: audit A10 / issue #152 only, under the lead's scope split.
- This CLEAN covers P5's package scope for this PR only.

The implementer reverted `913594cb`. The reviewer checked both tree objects.
This head and the Round 3 CLEAN head `1a96eee7` have the same tree:
`9ca04e1f1a4111ed9319e97e8bcc316fcf7edf52`.
The diff against the base changes only `crates/botster-core/tests/slow_real_core.rs`.
The Round 3 source review and focused evidence therefore apply to the unchanged A10 source.
P5-F1 and P5-F2 remain CLOSED. Audit A10 remains CLOSED for this scope.

The lead's recorded merge order assigns the shared guard fix to P3's `stage1/p3-guard-macos` before #162.
P5-F3 and P5-F4 follow that shared guard work. They remain unresolved there; they are not waived or claimed fixed here.
The integration reviewer received both findings and the exact rejected head.
No finding remains open within the restored A10-only diff.

The reviewer read the updated full PR body. Its reverted-change note records the failed Mac gate and the scope split.
The implementer's raw diagnostic description establishes one orphaned `tee` observation.
The implementer explicitly states that the diagnostic did not prove the proposed kill/fork mechanism.
This verdict makes no kernel-level root-cause claim.

The reviewer ran no builds, tests, mutation jobs, or gates.
The failed Mac gate on `1a96eee7` remains failed. This CLEAN does not establish a passing full gate.
After P3's guard fix merges, #162 must merge `origin/v1`, receive an exact-head delta review, and pass its gate.
This verdict does not accept that future merged head or close the other P5 work.

VERDICT: CLEAN

## PR #164 — Round 1

- Exact head: `c4afe5861abe96904ad80f9507b147c2da8ea98e`.
- Base: `144b0234fb632bcbb5176b17c2fe55f3239405df`.
- Scope: audit A1, A2, A4, A5, A7, and A9 in P5 deliverable 1.
- The reviewer ran no builds, tests, mutation jobs, or gates.

### Audit closure at this head

A1 and A2 remain OPEN. P5-F5 and P5-F6 cover the remaining A1 cases.
P5-F5 also leaves the A2 reservation incomplete for damaged file headers.
P5-F8 identifies another in-scope durable-row overwrite after an uncertain Create write.

A4 is CLOSED in source. The failed Starting write now completes each waiting Stop with `RegistryFailed`.
The new test checks both `Failed` and `Uncertain`, with the expected error derived from the injected storage error.
A5 is CLOSED in source. The driver opens without reading a clock, and only `pump(now)` supplies time to the machine.
The facade removes its clock read and bans clock reads through Clippy configuration.

A7 is CLOSED in source. The unknown-hello test sends a complete frame and waits for a wake before pumping Core.
It then observes link EOF through a channel with a marked deadline. It no longer polls a short read timeout.

The host machine closes A9's missing non-child exit observation through an injected identity probe.
The real edge uses the existing system identity probe. The testkit edge remains incorrect across handles; P5-F7 is OPEN.
The placeholder worker recovery remains P5 deliverable 2. These tests no longer certify `Lost(Other)` as adoption behavior.

### P5-F5 — CONTRACT — A1 still drops rows with damaged file headers

**Evidence:** `crates/botster-core-sys/src/storage.rs:156-169` lists only files whose header decoder returns a key.
A damaged key header still causes the storage edge to omit the file.
`crates/botster-core/src/real.rs:264-272` therefore returns no entry for that row.
`HostDriver::open` cannot reserve its ID, and `AdoptAll` cannot post its state.
The hashed filename does not preserve the ID when both the header and value are damaged.

Audit A1 explicitly includes this storage case. AD-1 and LC-11 require a state event for every row.
AD-2 requires `Lost(RegistryCorrupt)` and continued ID reservation for a corrupt row.
The new registry tests damage the value in an in-memory map; they retain the key and do not cover this file case.

**Required change:** Preserve each Core row's identity independently of its header and value.
Pass an attributable damaged row to the host instead of omitting it.
The host must post `Lost(RegistryCorrupt)` and reject Create for that ID until Remove.

The lead ruled on the storage layout after this review request.
Use a fixed directory for the kind and a reversible base32 ID split into components of at most 200 characters.
Traverse and create each component with `openat` and `mkdirat`, without a `PATH_MAX` dependency.
Fsync each created parent directory. Remove deletes the row, then removes empty directories on a best-effort basis.
A decodable path identifies a row. Other paths are foreign: count them, leave them unchanged, and do not block adoption.
Keep the existing configurable ID limit; do not add a fixed cap.
This ruling specifies the fix. It does not close the finding at this head.

**Closure evidence:** Start with bytes written by Core's own storage encoder.
Damage the header and value, reopen through the real storage edge, and observe the decoded ID's state event.
Check `Lost(RegistryCorrupt)`, `IdInUse`, unchanged row bytes after the refused Create, and ID reuse after Remove.
Derive boundary cases from the configured ID limit and the chosen path component bound.
Supply a failing baseline that reaches the contract assertion, plus the passing revised result.

Status: OPEN.

### P5-F6 — CONTRACT — A1 still skips a row whose session is already in memory

**Evidence:** `crates/botster-core-host/src/run.rs:645-646` returns immediately when the session table contains the row's ID.
Create a session, drain its events, and run the handle's first AdoptAll.
The durable row produces no new `SessionState` during that AdoptAll.
An earlier Create event does not satisfy AD-1's event for each row read by AdoptAll.
Audit A1 explicitly names colliding rows, and LC-11 repeats the requirement.

**Required change:** Post one current state for this row during AdoptAll while preserving the existing session instance.
Do not replace the session or replay earlier lifecycle transitions.

**Closure evidence:** Create through the public operation path on one handle and drain the Create events.
Run the first AdoptAll on that handle. Check one state event for the row and the successful AdoptAll completion.
Derive the expected ID and instance from the created session, and check that the instance remains unchanged.
Supply a failing baseline at the missing state assertion and a passing revised result.

Status: OPEN.

### P5-F7 — MEDIUM — The testkit reports a live worker from an earlier handle as absent

**Evidence:** `crates/botster-core-testkit/src/worker.rs:155-160` allocates a new process table for each spawner.
The new `identity_state` method at lines 244-249 searches only that table.
The shared `Workers` simulation retains workers from earlier handles, but a reopened handle cannot find their identities.
Remove therefore receives `Absent`, completes, and frees the ID while the simulated worker still lives.
The real edge and the new host unit model instead find the worker and end it.
The testkit no longer models the same edge facts across handles, despite the shared driver.

The integration reviewer independently reports this defect as K1 at the same head.
Their NOT CLEAN verdict commit is `3a1aa39`.

**Required change:** Share process identity and signal state across the `Workers` run.
Keep child exit notifications scoped to the handle that owns those children.
Do not give a reopened handle another handle's exit notifications.

**Closure evidence:** Drive the actual testkit driver through Start, host drop, reopen, AdoptAll, and Remove.
Check that Remove ends the earlier worker, reports the contract's unknown upload outcome, and frees the ID.
Check that identity probes and group signals reach that worker through the injected edges.
Supply a failing baseline that reaches the live-worker assertion and a passing revised result.
Obtain the integration review on the revised exact head.

Status: OPEN.

### P5-F8 — CONTRACT — An uncertain Create write can leave a row that a retry overwrites

**Evidence:** `crates/botster-core-host/src/flows.rs:692-700` treats every failed Create write as if no row was written.
It removes the in-memory session without reserving the ID in `unadopted`.
That set receives durable IDs only when the driver opens.

The real storage edge can return `Uncertain` after replacement.
`crates/botster-core-sys/src/storage.rs:118-128` documents this case and returns `Uncertain` after a directory sync error.
In that case, a retry on the same handle admits Create and replaces the row with a new instance.
This is an existing flow defect exposed by the A2 reservation review; it is not a regression introduced by this PR.

ID-1 requires uniqueness among registry rows. LC-3 requires `IdInUse` for a duplicate ID.
AD-7 requires Core to assume neither success nor failure for an uncertain write.
The current comment explicitly assumes failure.
The host test edge at `crates/botster-core-host/src/tests.rs:211-220` returns injected errors without applying the write.
That model cannot exercise a write which takes effect and then reports uncertainty.

**Required change:** Keep the uncertain ID reserved until authoritative registry reconciliation can resolve it.
Do not allow a same-handle retry to overwrite a row that the uncertain write can have created.
Keep the `RegistryFailed{uncertain: true}` completion and avoid claiming that the session was durably created.

**Closure evidence:** Inject a storage edge that applies Core's encoded row, then returns `Uncertain`.
Observe the Create error. Retry the same ID and check `IdInUse` with no second row write.
Reopen and check recovery from the row that the first write left.
Derive the expected row and instance from the first encoder output.
Supply a failing baseline at the duplicate admission assertion and a passing revised result.

Status: OPEN.

### Execution evidence and scope limits

The reviewer read the Linux focused log for `9ac2d6cbe7405541f1bfa1889aad20eb394c4367`:
`~/botster-sessions/gates/botster-core-stage1-p5-audit-contract-9ac2d6cb-linux-20261004-203844-39095.log`.
That head and the PR's first commit `4a59761` have the same tree, `883687a115d20d2b752bd88319c5213f7b263982`.
The log reports 420 passed, 654 skipped, and exit 0. It covers the earlier source changes, before the A7 change.

The reviewer also read the focused Mac log for the exact requested head:
`~/botster-sessions/gates/botster-core-stage1-p5-audit-contract-c4afe586-mac-20261004-214523-55738.log`.
It reports 15 passed, zero skipped, and exit 0 for `slow_real_core`, including the revised unknown-hello test.
These focused results do not establish a passing full gate or close the four source findings.
The reviewer read the full PR body. Its declared storage and testkit limitations do not waive audit A1 or A9.

This verdict covers this PR's P5 audit scope only. It does not close the remaining deliverable 1 work or P5 adoption.

VERDICT: NOT CLEAN (4 open)

## PR #164 — Round 2

- Exact head: `26c4c2ef58d7ac20b059487013da760bf34911eb`.
- Base: `144b0234fb632bcbb5176b17c2fe55f3239405df`.
- Scope: the Round 1 audit fixes and the new storage implementation.
- The review includes `c4afe586..e6d9f487` and the requested `e6d9f487..26c4c2ef` delta.
- The reviewer ran no builds, tests, mutation jobs, or gates.

### P5-F5 — MEDIUM — The header defect closes in source, but its regression fails before the contract assertion

The reversible path now preserves the key outside the value file.
The scanner lists a damaged value under that key, and the real edge returns its bytes to the host decoder.
The original missing-header source defect is CLOSED.
The remaining finding concerns regression proof; its severity changes from CONTRACT to MEDIUM.
P5-F9 separately covers durability defects in the new implementation.

**Evidence:** `crates/botster-core/tests/slow_real_core.rs:377-385` assumes the new `rows/session` directory exists.
The previous storage layout has no such directory, so this test fails during setup on the baseline.
It does not reach the missing `SessionState` assertion that must prove audit A1.
The test then replaces the value with literal invalid bytes at line 386 instead of damaging bytes derived from the written file.
The supplied log contains passing tests, but no failing baseline at the intended assertion.

**Required change:** Locate the session file from the file changes caused by Core's Create.
Do not require the revised layout to prepare the regression.
Read the Core-written bytes and damage those bytes, such as by a derived truncation.
Check exactly one `Lost(RegistryCorrupt)` event for the created ID, not only the presence of an event.
Retain the duplicate admission and foreign-file checks.
Check that the refused Create preserves the damaged bytes, then check ID reuse after Remove.
Supply the failing baseline at the missing state assertion and the passing revised result.

Status: OPEN (regression proof).

### P5-F6 — CONTRACT — A colliding row still posts no state while Create has not shown Created

The new same-handle test closes the case where Create has already completed.
The revised source still omits another reachable colliding-row case.

**Evidence:** `crates/botster-core-host/src/run.rs:645-650` posts only when `session.shown` is `Some`.
After a successful Create row write, the session remains at `CreatePhase::PostCreated` with `shown == None`.
AdoptAll can then read that durable row and run AdoptRow before the Create state step.
`ready()` offers operation steps before session steps at `run.rs:118-165`.
The injected scheduler can also select or defer these independent steps.
AdoptRow returns without posting, and `adopt_next_row` completes AdoptAll.
The later Create event does not supply the state before that adoption completes.
The DESIGN note explicitly counts the later Create event as the row's event, without enforcing this dependency.

**Required change:** Do not complete AdoptAll for this row before its state is posted.
Preserve the existing instance and the normal Create completion.
Do not replace the session or add a test-only scheduling rule.

**Closure evidence:** Use injected scheduling to pause Create after its row takes effect and before its Created event.
Run the first AdoptAll through that interleaving.
Check the row's state and instance before the AdoptAll completion, and check the eventual Create completion.
Derive the expected instance from the actual attempted row or operation result.
Retain the completed-Create case. Supply a failing baseline and a passing revised result at the contract assertion.

Status: OPEN.

### P5-F7 — CLOSED — The testkit shares identity and signal state across handles

`Workers::run_processes` now holds each worker's identity, process cell, and spawning owner.
Each spawner probes and signals through that shared table.
The Kill path sends the exit to the spawning owner's table; `poll_exit` still reads only the current owner's exits.
The new test drops and reopens the actual testkit driver, removes the earlier worker, and observes its absence.
The supplied Mac log reports that test passing.

The integration reviewer closed K1 at `e6d9f487`, verdict commit `e6078c1`.
Their review of `26c4c2ef`, commit `8bed899`, reports zero open integration findings.
They condition their CLEAN on the package CLEAN for that exact head.
P5-F7 is CLOSED. Audit A9 is CLOSED within this PR's teardown scope.

### P5-F8 — MEDIUM — The reservation closes in source, but the test does not apply the attempted uncertain write

The Create failure path now reserves an uncertain ID in `unadopted`.
AdoptAll reconciles that reservation against the rows it reads.
This closes the original same-handle overwrite source defect.
The remaining finding concerns regression proof; its severity changes from CONTRACT to MEDIUM.

**Evidence:** `crates/botster-core-host/src/tests/registry.rs:233-268` injects an error that does not write the attempted row.
It later inserts a row copied from a separate World for the `took_effect` case.
The test does not apply the actual `Action::WriteRow` bytes before returning `Uncertain`.
It also does not compare the attempted row, recovered instance, or write count after the retry.
It proves admission remains blocked, but it does not exercise the real applied-then-uncertain edge case from Round 1.

**Required change:** Let the injected storage edge apply the actual attempted row and then return `Uncertain`.
Keep the complementary case where the error has no effect.
Check the uncertain completion, duplicate refusal, unchanged row bytes, and absence of a second row write.
Reopen through the normal driver and check recovery of the first attempted instance.
Derive expected bytes and identity from that attempt.
Supply the failing baseline at the duplicate admission assertion and the passing revised result.

Status: OPEN (regression proof).

### P5-F9 — CONTRACT — Successful writes do not establish every ancestor directory's durability

**Evidence:** `crates/botster-core-sys/src/storage.rs:302-323` creates the data directory and the `rows` directory.
It never syncs their containing directories.
The row protocol syncs the leaf directory, and `child_dir` syncs parents below the `rows` root.
Those calls do not establish the `rows` entry's durability in the data directory or newly created data-directory ancestors.
A successful open and Create can therefore leave the registry's root outside the promised durability protocol.
This root gap existed before the rewrite and remains in the storage code that this PR now owns.

There is also a new retry gap at `storage.rs:105-110`.
After `mkdirat` succeeds, a failed parent sync leaves the new directory in place and returns an error.
A later write sees `EEXIST` and skips that parent sync.
The later leaf sync can succeed without establishing the ancestor entry's durability.
The later write then reports success.

AD-7 requires durable registry identity before a payload runs. DP-8 requires a persistent host epoch.
The lead's storage ruling requires a sync for every created parent directory.
A sync on an object does not guarantee its entry in its containing directory.
The Linux manual requires a separate sync for that directory entry. [fsync(2)](https://man7.org/linux/man-pages/man2/fsync.2.html)
This is a missing durability guarantee; the reviewer makes no claim that the supplied run lost data.

**Required change:** Include the data root and `rows` root in the directory durability protocol.
Handle all ancestors that Core creates.
Before reporting a successful retry, establish durability for a directory left by an earlier failed sync.
Do not treat `EEXIST` alone as proof that its parent entry is durable.
Use one production protocol with injected filesystem operations, without test branches.

**Closure evidence:** Inject a failure at the parent sync after successful directory creation, then retry the write.
Check the first failure and the later result through the production storage protocol.
Check that the retry syncs every required parent before it reports success.
Derive that parent set from the created path chain.
Cover fresh data and `rows` roots as well as an ID that creates multiple path components.
Supply a failing baseline at the missing sync guarantee and a passing revised result.

Status: OPEN.

### P5-F10 — LOW — The PR description still states removed limitations as current behavior

**Evidence:** The full PR body at this head still says `FileStorage::list_rows` drops damaged headers under SHA-256 names.
It still says the identity probe answers only for the current host.
Its cross-package summary calls this revision two small edits, despite the new shared process table and storage rewrite.
Appending a Round 2 section leaves these conflicting statements in the final review description.

**Required change:** Rewrite the description for the final implementation and current scope.
Remove the obsolete limitations and describe the remaining P5 adoption placeholder accurately.
Keep prior results with their exact heads and keep the failed slow result marked failed.
The new Prior art note names `data-encoding` and explains the directory-descriptor protocol; integration K2 closes in source documentation.

Status: OPEN.

### Execution evidence and scope limits

The reviewer read the raw Mac log:
`~/botster-sessions/gates/botster-core-stage1-p5-audit-contract-e6d9f487-mac-20261004-215950-7755.log`.
Its header names `e6d9f487a5c815e8d3abfd922b46d722af5db5b9`.
It reports 486 default-tier tests passed, with 654 skipped.
The slow result reports 75 passed, one failed, one timed out, and exit 100.
The A10 cleanup test failed. The shared guard test `parent_dies_before_fifo_reader` timed out after 2.006 seconds.
Both are known work in #162 and P3's #165; this review does not mark either result passed.
The new damaged-row, long-ID, existing-row, uncertain-Create, and testkit reopen tests passed on that earlier head.

The final `e6d9f487..26c4c2ef` source delta changes error mapping and the scan result type.
It maps errors without an OS code to `EIO` and removes the facade's zero-errno fallback.
The integration reviewer closed K3 for that delta. No execution result on `26c4c2ef` was supplied with this request.
A4, A5, and A7 remain CLOSED in source. A1 and A2 await the remaining findings above.
No mutation exclusion changed in this delta. Landing checks remain the implementer's responsibility.

VERDICT: NOT CLEAN (5 open)

## PR #164 — Round 3

- Exact head: `737b2017df84f2d7fa3641c7e91f065b13ecc6ff`.
- Base: `144b0234fb632bcbb5176b17c2fe55f3239405df`.
- Scope: `26c4c2ef..737b2017`, the revised PR description, and the five Round 2 findings.
- The reviewer ran no builds, tests, mutation jobs, or gates.

### P5-F5 — MEDIUM — The revised damage still preserves the old key header

The test now finds the file from the files added by Create.
It no longer assumes the new directory layout and now derives damaged bytes from the Core-written file.
Those parts of the Round 2 request are CLOSED.

**Remaining evidence:** `crates/botster-core/tests/slow_real_core.rs:380-387` retains the first half of the file.
Under the old storage layout, that half still contains the short key header for `session/s1`.
Only the much longer JSON value is truncated.
At `c4afe586`, the old storage therefore lists the key, and the repaired host value decoder posts `Lost(RegistryCorrupt)`.
The revised test does not distinguish the header defect from the value defect that Round 1 already fixed.
No failing baseline at the missing-header contract assertion was supplied.

**Required change:** Damage the complete file, or remove the header as well as the value, without assuming the old layout.
Start with the file that Core's Create adds and derive the damaged bytes from that file.
Check exactly one state for the row and preserve the duplicate admission check.
Supply a failing result on the old storage with the repaired host decoder, at the missing state assertion.
Then supply the passing revised result.

Status: OPEN (header-specific regression proof).

### P5-F6 — CLOSED — AdoptAll waits for the pending Create state

`ready()` now holds AdoptRow while its next row names a session whose state is not shown.
Once Create posts Created, AdoptRow posts the existing session's state and preserves its instance.
The dependency applies in the production machine and all injected schedules.

The new test makes Create write its row, then selects only offered work and prefers AdoptAll when available.
It checks both the Create state and the row state before the AdoptAll completion.
The previous same-handle test checks instance preservation after a completed Create.
The supplied exact-head log reports both tests passing.
The source comparison shows that the earlier skip fails the new event-order assertion.
P5-F6 is CLOSED.

### P5-F7 — CLOSED — The shared testkit process table remains intact

The delta does not change the shared identity and signal table or parent-scoped exit notifications.
The exact-head log again reports the testkit reopen regression passing.
The integration reviewer reports zero open integration findings at this head, verdict commit `2070167`.
They condition their CLEAN on this package's exact-head CLEAN.
P5-F7 remains CLOSED.

### P5-F8 — CLOSED — The injected write now takes effect before it reports uncertainty

The test edge's `fail_row_after_write` applies the actual WriteRow bytes before it reports the injected error.
The regression checks `RegistryFailed{uncertain: true}`, duplicate refusal, and an unchanged row-write count.
It decodes the attempted row and compares the recovered state event's instance with that row's instance.
The complementary no-effect case checks that registry reconciliation frees the ID.
The test now exercises the applied-then-uncertain case with values derived from the actual write.
The supplied exact-head log reports it passing.
The earlier flow admits the duplicate and fails the new refusal assertion.
P5-F8 is CLOSED.

### P5-F9 — CONTRACT — A retry still forgets a created data-directory ancestor

The row path now syncs its parent on both successful mkdir and `EEXIST`.
The open path also syncs the `rows` entry in the data directory.
Those source cases from Round 2 are CLOSED.

**Remaining evidence:** `crates/botster-core-sys/src/storage.rs:361-376` stops its missing-ancestor list at the first existing directory.
Consider an open for `/base/new/a/data`, with `/base` already present.
The first attempt creates `/base/new`, then fails while syncing `/base`.
That attempt leaves `new` present without establishing its entry's durability.
The retry sees `new` as existing, creates `a` and `data`, and syncs `new` and `a`.
It never syncs `/base` to repair the remaining obligation.
If the full requested path already exists on retry, the function syncs only its immediate parent.
It still cannot repair an earlier unsynced ancestor.

**Required change:** Preserve or re-establish every directory sync obligation across failed opens and later retries.
Do not use existence as proof of an ancestor entry's durability.
Keep one production protocol with injected filesystem operations.

**Closure evidence:** Fail the sync after creating an intermediate ancestor, then retry the same open.
Check that the retry syncs the failed ancestor's parent before open or a later row write reports success.
Derive the required parents from the requested path and the directories created by the first attempt.
The existing read-back tests do not cover this failure.
Supply the failing baseline and the passing revised result through the same protocol.

Status: OPEN.

### P5-F10 — LOW — The PR body is current, but its referenced DESIGN decision remains stale

The reviewer read the rewritten full PR body at this head.
It removes the header and per-host process limitations and describes the current scope.
It names the earlier slow failures separately from the new focused passing result.
The original PR-body finding is CLOSED.

**Remaining evidence:** `crates/botster-core-host/DESIGN.md` still says an unshown Created state posts the row state through Create.
The revised code instead waits for Create's state, then posts a separate AdoptAll row state.
The new regression checks both events before the adoption completion.

**Required change:** Update that DESIGN decision to describe the enforced dependency and separate row event.

Status: OPEN (documentation residue).

### Execution evidence and scope limits

The reviewer read the raw exact-head Mac log:
`~/botster-sessions/gates/botster-core-stage1-p5-audit-contract-737b2017-mac-20261004-220651-25713.log`.
Its header names `737b2017df84f2d7fa3641c7e91f065b13ecc6ff`.
It reports 487 default-tier tests passed, with 654 skipped.
The selected slow targets report 56 passed, zero skipped, and exit 0.
This selected run excludes the shared guard target that timed out in the earlier wider run.
It does not establish a passing full gate.

A4, A5, A7, and A9 remain CLOSED within this PR's audit scope.
A1 awaits P5-F5 and the remaining documentation; the new storage durability finding P5-F9 also remains open.
The reviewer does not accept a future merged head or close P5 deliverable 2.

VERDICT: NOT CLEAN (3 open)

## PR #164 — Round 4

- Exact head: `1b1340cf80ed1f1aa3b05fba7d2e524c8b30de1a`.
- Base: `144b0234fb632bcbb5176b17c2fe55f3239405df`.
- Scope: `737b2017..1b1340cf`, the three Round 3 findings, and integration K4 in the storage code.
- The reviewer ran no builds, tests, mutation jobs, or gates.

### P5-F5 — CLOSED — The regression destroys the key header and the value

The real test locates the file added by Core's Create and inverts every byte of that file.
It derives the damage from the actual encoder output and assumes no registry layout.
The old storage's small key length becomes a length larger than the file; its decoder returns no key.
The baseline therefore reaches AdoptAll without a row state and fails the missing-state assertion.
The revised scanner preserves the path's key and the host posts `Lost(RegistryCorrupt)`.
The supplied exact-head log reports this test passing.
The reviewer established the failing baseline behavior by source comparison and ran no test.
The separate registry tests check one event per row, corrupt Remove, and ID reuse.
P5-F5 is CLOSED. Audit A1's damaged-header case is CLOSED in this scope.

### P5-F9 — CLOSED — The retry now syncs the remaining ancestor entries

The production `create_durably` helper accepts an injected directory sync.
It creates the path, resolves its canonical ancestors, and syncs each containing directory through that function.
A retry now includes an ancestor left by an earlier failed open, even when the full path already exists.
The row path retains its sync on successful mkdir and `EEXIST`; the open path retains the `rows` parent sync.

The new regression injects an I/O error at the base sync and then retries through the same helper.
It derives the required parent paths from the requested directory chain.
The retry trace must include the base whose sync failed and the intermediate parents.
The earlier immediate-parent-only retry fails that required-parent assertion.
The supplied exact-head log reports this test passing.
P5-F9's missing durability obligations are CLOSED at this head.
Integration K4 separately identifies an access regression in the broader sync operation.
Its fix must preserve these durability obligations.

### P5-F10 — CLOSED — The description and DESIGN decision match the implementation

The DESIGN note now states that the row waits for Create's Created event, then posts its own state before AdoptAll completes.
The reviewer read the full updated PR body at the exact head.
It describes the current storage protocol and gives each supplied result with its head.
It marks the current slow result failed and names the known A10 dependency.
P5-F10 is CLOSED.

### Integration K4 — LOW — The broader ancestor sync adds an access requirement

The integration reviewer reports K4 at this exact head, verdict commit `2aefb09`.
`create_durably` now opens and syncs every canonical ancestor up to the root.
An unchanged execute-only ancestor permits path traversal but cannot be opened for reading.
The new open therefore fails for a path that previously worked.
This finding affects P5's storage scope and remains open here.

**Required change:** Resolve the access regression with a documented boundary or tolerance rule and an injected regression.
Keep successful parent syncs for entries Core creates or can have left after a failed open.
Do not silently discard an unresolved sync obligation on `EACCES`.
The reviewer sent the lead a QUESTION about the supported durability and access boundary.
The integration reviewer must review the final exact head.

Status: OPEN.

### Execution evidence and scope limits

The reviewer read the raw exact-head Mac log:
`~/botster-sessions/gates/botster-core-stage1-p5-audit-contract-1b1340cf-mac-20261004-221058-38226.log`.
It reports 487 default-tier tests passed, with 654 skipped.
The selected slow targets report 56 passed, one failed, zero skipped, and exit 100.
The known A10 cleanup test failed. The new damaged-row and ancestor-retry tests passed.
This result remains failed; it does not establish a passing full gate.

P5-F5 through P5-F10 are CLOSED at this head.
Audit A1, A2, A4, A5, A7, and A9 are CLOSED within this PR's stated audit scope.
K4 prevents a CLEAN verdict until the remaining access rule closes.
The reviewer does not close the other P5 audit deliverables or P5 adoption.

VERDICT: NOT CLEAN (1 open)

## PR #164 — Round 5

- Exact head: `428c783967ca6e67d7cccb36db8336eb96aef004`.
- Base: `144b0234fb632bcbb5176b17c2fe55f3239405df`.
- Scope: `1b1340cf..428c7839` and the lead's K4 durability-boundary ruling.
- The reviewer ran no builds, tests, mutation jobs, or gates.

### Integration K4 — LOW — The lead replaced this head's writable-ancestor rule

The revision injects both the write-access check and the directory sync.
It stops the ancestor walk at the first directory that the host cannot write.
Its tests check an inaccessible ancestor above that boundary and an error below it.
The integration reviewer accepted that rule at this head, verdict commit `87ed24c`.

The lead then issued a different boundary in message `msg_plugin-w_1791177356_457755`.
That ruling supersedes both the full ancestor walk and this writable-ancestor walk:

1. Core creates at most the final `data_dir` component through a non-recursive mkdir.
2. A missing parent fails open with the existing typed registry I/O error.
3. Core syncs `data_dir` and its immediate parent on every open.
4. Core syncs no other ancestor. The host provides those ancestors and owns their durability.
5. The immediate parent must be openable for the required sync. Otherwise open fails and claims no durability.

This head still creates ancestors recursively and uses write access to select which ancestors to sync.
It therefore does not implement the new ruling.
The reviewer relayed the exact boundary to the implementer and integration reviewer.
No earlier finding is waived. The ruling narrows the directory creation responsibility and removes the ancestor ambiguity.

**Required change:** Implement the lead's boundary through the same injected production path.
Document the parent existence and access requirements in rustdoc and DESIGN.md.
Use the existing typed I/O error; ask the lead if no existing code fits.
Test a missing parent, an unreadable immediate parent, and a retry after a failed parent sync.
Observe the retry's parent sync through the injected edge.
Also test a nested data directory below an execute-only grandparent.
Obtain both reviews on the revised exact head.

Status: OPEN (lead ruling pending implementation).

### Evidence and scope limits

The reviewer read the raw exact-head Mac log:
`~/botster-sessions/gates/botster-core-stage1-p5-audit-contract-428c7839-mac-20261004-221459-45851.log`.
It reports 487 default-tier tests passed, with 654 skipped.
The selected slow targets report 58 passed, zero skipped, and exit 0.
The two ancestor tests passed. These results cover the superseded rule and are not proof of the lead's new boundary.
The selected result is not a full gate. The known A10 and shared guard dependencies still require their merge sequence.

P5-F5 through P5-F10 remain CLOSED for the unchanged source in this head.
The remaining K4 boundary prevents CLEAN. The rest of P5 deliverable 1 and P5 adoption remain separate work.

VERDICT: NOT CLEAN (1 open)

## PR #164 — Round 6

- Exact head: `1bcbb38520440ee7e98910373926156ff886edd4`.
- Base: `144b0234fb632bcbb5176b17c2fe55f3239405df`.
- Scope: `428c7839..1bcbb385`, the lead's directory ruling, and the new tests and documentation.
- The reviewer ran no builds, tests, mutation jobs, or gates.

### Integration K4 — CLOSED — The source implements the lead's directory boundary

`create_data_dir` uses a non-recursive mkdir and tolerates an existing final component.
It checks the data directory's safety, then syncs its immediate parent through the injected sync.
It performs that sync on every open, including a retry after a failed sync.
`DataDir::open` syncs the data directory after creating `rows`, also on every open.
It performs no sync above the immediate parent.
The former write-access check and ancestor walk are removed.

The facade maps `OpenError::Io` to `RegistryFailed{uncertain: false}`.
The DESIGN note and `DataDir::open` rustdoc state the host's parent requirements.
The new cases cover missing parent, unreadable parent, parent-sync retry, and an execute-only grandparent.
The retry test observes the actual parent through the injected sync and derives its expected path from the temporary root.
K4 is CLOSED in source. The integration reviewer confirms this closure at the exact head, verdict commit `b95fb90`.

### P5-F11 — LOW — The grandparent test fixes an initial epoch that the contract leaves open

**Evidence:** `crates/botster-core-sys/src/storage.rs:701-703` maps the open result to its epoch and asserts literal `1`.
The test's subject is successful open below an execute-only grandparent.
The lead's K4 ruling requires that success. It does not fix the first epoch.
DP-8 requires strict increase across opens; it also does not fix the first epoch.
The assertion therefore pins an implementation value instead of contract-visible behavior with a derived expectation.
This violates the user's test-quality requirement.

**Required change:** Check successful open without asserting a fixed first epoch.
If an epoch comparison is necessary, derive it from another observed open rather than a literal.
Keep the execute-only fixture and restore its permissions before the assertion.

Status: OPEN.

### Integration K6 — LOW — The public open documentation omits the new parent requirements

The integration reviewer reports K6 at the same head.
`crates/botster-core/src/lib.rs:62-67` documents `Core::open` but does not state either new parent requirement.
The Hub uses this public facade, not the system crate's `DataDir` documentation.

**Required change:** Document that the parent must exist and must be openable for its required sync.
State that Core creates only the final data-directory component and reports the existing typed error when a requirement fails.

Status: OPEN.

### Integration K7 — LOW — The unreadable-parent test accepts an unexpected successful open

The integration reviewer reports K7 at the same head.
`crates/botster-core-sys/src/storage.rs:659-663` accepts `Ok(_)` without checking that the process runs as root.
A missing parent sync could therefore make this permission regression pass for an ordinary user.

**Required change:** Assert `PermissionDenied` for the unprivileged fixture.
If root makes that fixture inapplicable, identify that condition explicitly and report the skip visibly.
Do not accept successful open as a substitute for the required failure assertion.
Obtain the integration review on the revised exact head.

Status: OPEN.

### Evidence and scope limits

The reviewer read the raw exact-head Mac log:
`~/botster-sessions/gates/botster-core-stage1-p5-audit-contract-1bcbb385-mac-20261004-221745-54622.log`.
It reports 487 default-tier tests passed, with 654 skipped.
The selected slow targets report 59 passed, one timed out, zero skipped, and exit 100.
The shared guard test `common::process_guard::parent_dies_before_fifo_reader` timed out after 2.003 seconds.
All four new storage cases passed. This selected run remains failed and is not a passing full gate.

P5-F5 through P5-F10 remain CLOSED. Audit A1, A2, A4, A5, A7, and A9 remain CLOSED in this PR's audit scope.
The three LOW findings above prevent CLEAN. The other P5 audit deliverables and P5 adoption remain separate work.

VERDICT: NOT CLEAN (3 open)

## PR #164 — Round 7

- Exact head: `b8b37a6d1161b83138a5e5e0e73bb68fc57e2db7`.
- Base: `144b0234fb632bcbb5176b17c2fe55f3239405df`.
- Scope: audit A1, A2, A4, A5, A7, and A9, plus the storage and testkit fixes reviewed in this PR.
- This CLEAN covers P5's package scope for this PR only.
- The reviewer ran no builds, tests, mutation jobs, or gates.

### Closure of the final delta

The reviewer inspected both commits in `1bcbb385..b8b37a6d`.
They change only the public open documentation and the storage tests.
The production directory protocol remains the lead's final-component-only creation rule.
It syncs the data directory and its immediate parent on every open and syncs no higher ancestor.

P5-F11 is CLOSED. The execute-only grandparent test now checks successful open without fixing an initial epoch.
Integration K6 is CLOSED. `Core::open` now documents parent existence, parent read access, and the existing `RegistryFailed` outcome.
Integration K7 is CLOSED. The permission helper requires `PermissionDenied` for an unprivileged process.
It explicitly identifies root and prints the reason when that permission fixture is inapplicable.
The implementer also removes the same unchecked-success arm from two older permission tests.

P5-F5 through P5-F10 remain CLOSED under the prior source reviews.
The final tree retains the full-file damage regression, the pending-Create dependency, and the applied-then-uncertain write regression.
The shared testkit process table still probes and signals workers across handles while keeping exits with the spawning handle.
No new mutation exclusion or production test branch was added by this delta.
No finding remains open within this PR's package scope, including LOW findings.

### Audit closure and integration review

Audit A1 is CLOSED: rejected values and damaged files remain attributable, colliding rows post a state, and the tests no longer certify the skip.
Audit A2 is CLOSED: the open path reserves durable IDs, and an uncertain Create write cannot permit a same-handle overwrite.
Audit A4 is CLOSED: Stop waiters receive the failed Start's typed registry error.
Audit A5 is CLOSED: the facade and driver read no clock; pump supplies the machine's time.
Audit A7 is CLOSED: the unknown-hello test waits for real wake and EOF events with marked deadlines.
Audit A9 is CLOSED: Remove probes an adopted non-child identity and observes teardown without relying on a child-reaper event.

The integration reviewer reported zero open findings at this exact head, verdict commit `cc389c2`.
After receiving package verdict commit `89893b23`, they issued terminal CLEAN at the same head, verdict commit `9edbc27`.
They sent both verdict commits to the lead. The package and integration reviews now both report CLEAN for this exact head.

### Evidence and scope limits

The reviewer read the full updated PR body and the raw exact-head Mac log:
`~/botster-sessions/gates/botster-core-stage1-p5-audit-contract-b8b37a6d-mac-20261004-222024-65207.log`.
Its header names the exact head and base above.
It records clean format and Clippy checks, 487 default-tier tests passed with 654 skipped, and 60 selected slow tests passed.
The selected slow result has zero skipped tests and exit 0.
The parent requirements, retry, and execute-only grandparent cases passed.

This is focused execution evidence. It is not a passing full landing gate or mutation result.
Earlier failed and timed-out results remain failed and timed out; this review does not reclassify them.
The lead's merge sequence still requires the shared guard and A10 dependencies before the final merged-head gate.
Any new merged head requires its own delta review.

This verdict does not close audit A10's separate PR, issues #155 or #157, the P1 part of #156, or P5 deliverable 2.
The declared live-worker adoption placeholder remains P5 deliverable 2 and is not certified as AD-1 recovery behavior.

VERDICT: CLEAN

## PR #162 — Round 6

- Exact head: `59cda32f721f974dee8f8aeea1fef9472afe1d35`.
- Base: `144b0234fb632bcbb5176b17c2fe55f3239405df`.
- Scope: the merge of P3's guard head `c03bcfb181d21cdf805b752a790359f63b9ef7b9`, plus the C1 fix.
- The reviewer ran no builds, tests, mutation jobs, or gates.

The lead amended the pause to permit this review and the later PR #164 merge review.
The reviewer compared this tree with P3's exact guard head.
Only the A10 test and its common module differ from that tree.
The A10 fixture still keeps the observed FIFO open in the shell and its blocked child.
The fixture uses no CPU while blocked, and production alone reaps the worker.
P5-F1 and P5-F2 remain CLOSED for the A10 fixture.

### Integration C1 — CLOSED — The outer deadline and panic result cover cleanup

The cleanup thread catches the guard's panic and sends its result to the main test.
The main test resumes that panic with its original report.
The outer deadline derives from `2 * process_guard::cleanup::CLEANUP`.
It therefore allows the anchor's cleanup deadline and time to report its result.
The EOF observation retains its separate marked deadline.
The integration reviewer reports zero open integration findings at this head, verdict commit `e39b6b4`.

### P5-F4 — MEDIUM — The merged guard retains the reported unbounded waits

Round 5 transferred P5-F4 to P3's shared guard work. It did not waive this finding.
The merged tree still contains both waits named in Round 4:

- `crates/botster-core-sys/tests/common/process_guard.rs:280-282` reads readiness directly with `read_line`.
- `Parent::drop` calls `child.wait()` at line 237 without a deadline.
- `parent_dies_before_fifo_reader` drops that parent at line 289 before calling the bounded EOF helper at line 290.

The later EOF deadline cannot bound either earlier wait.
The file already has a bounded `first_line` helper, but this caller does not use it.
The shared cleanup module also has bounded observation of a test-owned child's exit.
Neither mechanism covers this parent cleanup.
P3's CLEAN on the dependency does not close these source facts.
BUILD.md testing rule 5 and the user's real-process rules still apply.

**Required change:** Bound the readiness read and test-owned parent cleanup with marked deadlines.
Preserve the EOF observation and production reaper ownership.
Check the other changed guard self-tests for the same wait-coverage error.
The lead permits one combined gate after both source reviews report CLEAN; this finding does not request an extra gate.

Status: OPEN.

### P5-F12 — LOW — The PR description still describes the earlier head

The reviewer read the full PR description through `gh pr view`.
It names head `1a96eee7`, claims one changed file, and describes a ten-second outer cleanup deadline.
Its final section says the guard merge remains a future step.
This exact head already carries the shared guard and uses the derived C1 deadline.
The description therefore does not describe the change that will land.

**Required change:** Rewrite the description around the final combined scope and exact head.
State the current cleanup deadline and panic propagation.
Keep earlier failed evidence identified as failed evidence.

Status: OPEN.

### Evidence and scope limits

The implementer supplied no execution result for this exact head and states that no focused run occurred.
The lead permits one combined gate after both exact-head source reviews report CLEAN.
This review neither runs that gate nor substitutes older execution evidence for it.
P5-F3's former quiet-group precondition is absent from the merged guard.
The shared cleanup now lists members, signals the reserved group, and waits for exit events through injected edges.
This source change addresses the precondition defect; it does not close P5-F4's separate test waits.

The two findings above prevent CLEAN on this combined head.
This verdict does not close PR #164's future merge, the other P5 audit work, or P5 adoption.

VERDICT: NOT CLEAN (2 open)

## PR #162 — Round 7

- Exact head: `59cda32f721f974dee8f8aeea1fef9472afe1d35` (unchanged).
- Base: `144b0234fb632bcbb5176b17c2fe55f3239405df`.
- Scope: the revised PR description and correction of the shared guard review status.
- The reviewer ran no builds, tests, mutation jobs, or gates.

P5-F12 is CLOSED. The reviewer read the full rewritten description and verified its unchanged exact head.
The description identifies the combined guard scope, merge commit, C1 fix, derived deadline, and panic propagation.
It identifies the earlier failed Mac result as failed evidence and states that this head has no execution result.
It also records the remaining P5-F4 / integration C2 finding and P3's assigned fix.

The integration reviewer confirmed that their earlier #165 CLEAN missed P5-F4.
They opened C2 MEDIUM on this exact combined head, verdict commit `9b9cc72`.
That correction supersedes the zero-open integration status recorded in Round 6.
The implementer reports that the lead assigned the fix to P3 in #165.
PR #162 must merge P3's revised CLEAN head and receive another exact-head review before its combined gate.

C1 and P5-F12 are CLOSED. P5-F4 remains OPEN for the source reasons in Round 6.
The reviewer does not accept the future merge or close the other P5 work.

VERDICT: NOT CLEAN (1 open)


## PR #165 — Round 2

- Exact head: `88faedde845c53b62690a4648045988e63d67dd3`.
- Base: `144b0234fb632bcbb5176b17c2fe55f3239405df`.
- Scope: the delta from `71195e7209cc0cbee03bedb52eda3f2b83db2585`, plus the retained P5-F4 fix.
- The reviewer ran no builds, tests, mutation jobs, or gates.

### C3 / P3-F45 — CLOSED — The early-exit test bounds both waits

`an_early_exit_keeps_the_group_owned_until_cleanup` reads readiness through `first_line`.
The helper applies the existing `CLEANUP` deadline and reports a failed read.
The test obtains the leader's status through `cleanup::Owned::status`.
That owner observes exit with `waitid(WNOWAIT)` under a marked deadline before reaping the leader.
The anchor still holds the group after the test reaps the leader.
The test checks the anchor's group before cleanup and observes EOF afterward.
The two other changed self-tests also use the bounded status method.

### C4 / P3-F46 — CLOSED — FIFO members replace the sleep loops

Both self-tests now use `/bin/cat` blocked on a FIFO with no writer.
The members retain the observed pipe until they end.
The early-exit fixture keeps a descendant after its shell leader exits.
The panic fixture requires no readiness indication before cleanup starts.
The delta adds no sleep, polling loop, timeout increase, or production test hook.

### P5-F4 — CLOSED on this dependency head

`parent_dies_before_fifo_reader` retains the bounded `first_line` readiness read.
Its `cleanup::Owned` parent retains bounded exit observation and reap after the test kills the parent.
The EOF check remains separate and bounded.
These changes close the source waits recorded in #162 Round 6.
The next combined #162 head must retain these changes before it can receive CLEAN.

### P5-F13 — CLOSED — The PR description matches the landing order

The reviewer found that the description said #165 lands before #162 and #163.
The reviewer sent the correction directly to P3.
P3 updated the description without changing the submitted head.
The reviewer read the updated description and verified its unchanged head.
It now states that #165 folds into #162 for one combined gate.
It describes the bounded readiness and child-exit waits and the FIFO fixtures.
It names the current focused Mac result and identifies the older result by its older head.
The Prior art section records the selected libraries, rejected approaches, and reason for the custom cleanup rounds.
No finding remains open within this review scope.

### Evidence and scope limits

The reviewer read the raw Mac log for the exact submitted head:
`~/botster-sessions/gates/botster-core-stage1-p3-guard-macos-88faedde-pool-20261008-204257-69794.log`.
The pool job is `jobq-botster-core-88faedde-20261008204257-5ca7`.
The log reports successful Clippy and worker prebuild steps.
It reports 168 tests passed with zero skipped, followed by 15 selected guard tests passed with nine skipped.
The command exits with status 0.
This focused result does not establish a full landing gate or Linux execution.
The combined #162 head still needs its source reviews and its authorized gate.

VERDICT: CLEAN


## PR #165 — Round 3

- Exact head: `7e20c5fa2cbdc7855401ad9b71c8c941e064d878`.
- Base: `144b0234fb632bcbb5176b17c2fe55f3239405df`.
- Scope: the delta from accepted `88faedde845c53b62690a4648045988e63d67dd3`.
- The reviewer ran no builds, tests, mutation jobs, or gates.

### C5 / P3-F47 — CLOSED — The payload failure fixture owns and bounds its child

`a_payload_cleanup_that_cannot_finish_fails_through_the_guard` now wraps its fixture child in `cleanup::Owned`.
It obtains the child's status through the existing bounded exit observation before reaping the child.
The owner also kills and reaps the fixture child during unwinding.
This self-test has no production reaper; the owner applies only to its test-owned child.
Production reaper ownership remains unchanged in the real payload tests.

Both failure fixtures remove the `/bin/cat` stdout redirection to `/dev/null`.
The blocked member therefore retains the observed pipe until it ends.
The EOF assertion now observes the blocked member's end before the test obtains its child status.
The payload fixture retains its explicit release, expected guard failure, and failure-report assertion.
The shared FIFO helper changes visibility only and preserves the existing fixture behavior.

C3/P3-F45, C4/P3-F46, and P5-F4 retain their source fixes from Round 2.
The delta changes no timeout value, production code, transcript, or mutation exclusion.
No finding remains open in this dependency review scope.

### Description and evidence

The reviewer read the updated PR description and verified the exact submitted head.
The description retains the combined #162 landing rule and the Prior art section.
It describes the payload report over the guard socket and identifies older execution results by their heads.

The reviewer read the raw exact-head focused Mac log:
`~/botster-sessions/gates/botster-core-stage1-p3-guard-macos-7e20c5fa-pool-20261008-204548-78525.log`.
Job: `jobq-botster-core-7e20c5fa-20261008204548-8f21`.
The log reports successful Clippy and worker prebuild steps.
It reports 168 tests passed with zero skipped, then 15 selected guard tests passed with nine skipped; exit 0.
The changed failure fixtures pass in every binary that includes them.
This focused result does not establish a full landing gate or Linux execution.
The combined #162 head still needs its own delta reviews and its authorized gate.

VERDICT: CLEAN


## PR #165 — Round 4 — Correction of Round 3

- Exact head: `7e20c5fa2cbdc7855401ad9b71c8c941e064d878` (unchanged).
- Base: `144b0234fb632bcbb5176b17c2fe55f3239405df`.
- Scope: the separate real-loop path identified by P3's reviewer as F39.
- The reviewer ran no builds, tests, mutation jobs, or gates.

### P3-F39 — LOW — The real-loop wait omits cleanup completion time

The reviewer read P3 verdict commit `2013055b16be3ddcaf385efc187f00161d95a64a`.
The reviewer then independently checked `crates/botster-worker/tests/common/driver_edges.rs:437-455` at this head.
`pty_events_resume_reads_after_would_block` takes `Driver` out of `Bounded` and runs it on another thread.
The test releases the payload guard before sending Remove.
Its outer retirement wait remains ten seconds, equal to the member's `CLEANUP` limit.
It therefore omits the completion and result-delivery allowance supplied by `Bounded::drop` through `2 * CLEANUP`.
This wait can expire before the cleanup result arrives.

**Required change:** Apply the accepted outer-wait composition rule to this separate real-loop wait.
Preserve the inner cleanup limit and the release-before-production, report-after-production order.

Status: OPEN at this head. The reviewer sent the finding directly to P3.
C5/P3-F47 and the description correction remain closed.
Round 3's dependency CLEAN is superseded by this correction.
The passing focused result does not close this source finding.
The separate F39/G2 merge duty in #163 remains outside this correction.

VERDICT: NOT CLEAN (1 open)

## PR #165 — Round 5

- Exact head: `9eaea51ccb47230f4ee19e14056c17dfb607a826`.
- Base: `144b0234fb632bcbb5176b17c2fe55f3239405df`.
- Scope: the delta from `7e20c5fa2cbdc7855401ad9b71c8c941e064d878` and closure of its real-loop F39 finding.
- The reviewer ran no builds, tests, mutation jobs, or gates.

### P3-F39 — CLOSED in #165 — The outer wait includes cleanup completion

The one-file delta changes the real-loop retirement wait to `recv_timeout(2 * CLEANUP)`.
It applies the same accepted composition rule as `Bounded::drop`.
The inner `CLEANUP` limit remains unchanged.
The test still releases the payload guard before Remove, receives the driver result, joins the thread, and reads the guard report.
The delta changes no production code or transcript.
C3/P3-F45, C4/P3-F46, C5/P3-F47, and P5-F4 retain their source fixes.
No finding remains open within this #165 review scope.
This closure does not close F39/G2 in #163's separate merge delta.

### Description and evidence

The reviewer read the updated PR description and verified the exact submitted head.
It describes the outer-wait composition and limits its bounded-observation claim to the named fixture observations.
It retains the combined #162 landing rule and the Prior art section.

The reviewer read the raw exact-head focused Mac log:
`~/botster-sessions/gates/botster-core-stage1-p3-guard-macos-9eaea51c-pool-20261008-204745-82739.log`.
Job: `jobq-botster-core-9eaea51c-20261008204745-11ae`.
The log reports successful Clippy and worker prebuild steps.
It reports 168 tests passed with zero skipped, then 15 selected guard tests passed with nine skipped; exit 0.
The changed real-loop test passes.
This focused result does not establish a full landing gate or Linux execution.
The combined #162 head still needs its own delta reviews and authorized gate.

VERDICT: CLEAN


## PR #165 — Round 6 — Correction of Round 5

- Exact head: `9eaea51ccb47230f4ee19e14056c17dfb607a826` (unchanged).
- Base: `144b0234fb632bcbb5176b17c2fe55f3239405df`.
- Scope: integration C7 from verdict `e91cd835851668f9c95d3e8ab67c3bdff759ebeb`.
- The reviewer ran no builds, tests, mutation jobs, or gates.

### C7 — LOW — The payload panic test omits cleanup completion time

The reviewer independently checked `crates/botster-core-sys/tests/slow_payload.rs:312-334` at this head.
`a_panic_ends_the_payload_while_it_waits_for_input` waits ten seconds for panic cleanup on a helper thread.
That cleanup drops `GuardedPayload`, which releases the guard, runs production cleanup, and reads the guard report.
The member can use the full ten-second `CLEANUP` interval before production finishes its reap.
The outer wait therefore omits the reap and report allowance required by the accepted composition rule.

**Required change:** Derive this outer wait from `2 * process_guard::cleanup::CLEANUP`.
Keep the inner limit and the drop order unchanged.

Status: OPEN at this head. The reviewer sent the finding directly to P3.
Round 5's dependency CLEAN is superseded by this correction.
The real-loop F39 closure remains valid.

VERDICT: NOT CLEAN (1 open)

## PR #165 — Round 7

- Exact head: `47ae53a79b95e5499b8548c456a2fbaceacba992`.
- Base: `144b0234fb632bcbb5176b17c2fe55f3239405df`.
- Scope: the delta from `9eaea51ccb47230f4ee19e14056c17dfb607a826` and closure of C7.
- The reviewer ran no builds, tests, mutation jobs, or gates.

### C7 — CLOSED — The payload panic wait includes cleanup completion

The one-file delta uses `recv_timeout(2 * process_guard::cleanup::CLEANUP)` for the outer panic-cleanup wait.
It retains the deadline marker and explains the guard, reap, and report sequence.
The inner limit and the drop order remain unchanged.
The test still checks that the panic occurred and joins the helper after receiving its result.
The reviewer also checked the other channel waits in the guard consumers.
Those waits observe individual events rather than enclosing this cleanup sequence.
The previously reviewed source fixes remain present.
No finding remains open within this #165 review scope.
The separate F39/G2 duty in #163 remains outside this closure.

### Description and evidence

The reviewer read the updated PR description and verified the exact submitted head.
It names all three outer cleanup waits and records the separate #163 guard-drop duty.
It retains the combined #162 landing rule and the Prior art section.

The reviewer read the raw exact-head focused Mac log:
`~/botster-sessions/gates/botster-core-stage1-p3-guard-macos-47ae53a7-pool-20261008-204957-86837.log`.
Job: `jobq-botster-core-47ae53a7-20261008204957-4a68`.
The log reports successful Clippy and worker prebuild steps.
It reports 168 tests passed with zero skipped, then 15 selected guard tests passed with nine skipped; exit 0.
The changed payload panic test passes.
This focused result does not establish a full landing gate or Linux execution.
The combined #162 head still needs its own delta reviews and authorized gate.

VERDICT: CLEAN


## PR #162 — Round 8

- Exact head: `869f153068812002ddce401c823bdd6c617a3a4b`.
- Base: `144b0234fb632bcbb5176b17c2fe55f3239405df`.
- Parents: `59cda32f721f974dee8f8aeea1fef9472afe1d35` and accepted #165 head `47ae53a79b95e5499b8548c456a2fbaceacba992`.
- Scope: the combined merge delta and its P5 test boundary.
- The reviewer ran no builds, tests, mutation jobs, or gates.

### Source closure

The reviewer read the complete five-file delta from the first parent.
It contains the reviewed #165 fixes, with no additional source edit.
The reviewer compared the whole tree with the accepted #165 head.
Only `crates/botster-core/tests/common/mod.rs` and `crates/botster-core/tests/slow_real_core.rs` differ.
Both files match the previously reviewed #162 head exactly.
The reviewer checked the common module's shared guard import, construction, and shell prefix.
The merged helper visibility and call paths remain compatible with those users.

P5-F4/C2 is CLOSED in this combined head.
The parent-death readiness read and parent cleanup use the bounded observation paths accepted in #165.
C3, C4, C5, C6/F39, and C7 retain their reviewed dependency fixes.
P5-F1, P5-F2, and C1 retain their closures in the unchanged A10 test.
The test observes FIFO EOF, leaves worker reaping to production, and propagates guard failure to the main test.
Its outer cleanup wait derives from `2 * CLEANUP`.
No source finding remains open in this merge scope.
The separate #163 F39/G2 duty remains outside this closure.

### P5-F12 — LOW — REOPENED — The description still names the previous combined head

The reviewer read the PR description through `gh pr view` and verified the current remote head.
The description still names `59cda32f721f974dee8f8aeea1fef9472afe1d35` as its head.
It names `c03bcfb` as the dependency and says P5-F4/C2 remains open pending a future guard merge.
This head already merges accepted `47ae53a79b95e5499b8548c456a2fbaceacba992` and closes those source waits.
The description therefore does not describe the current combined change.

**Required change:** Name the current head and its accepted dependency.
Describe the final bounded waits and retained A10 proof.
State that P5-F4/C2 closes in the merged source and that the combined gate remains pending.
Preserve older evidence with its original head attribution.
This description correction needs no product commit or execution run.

Status: OPEN. The reviewer sent the finding directly to P5.
The existing Prior art section remains present.
No exact-head execution result was supplied for the combined head.
The dependency's focused Mac result does not substitute for the required combined Linux gate.

VERDICT: NOT CLEAN (1 open)


## PR #162 — Round 9

- Exact head: `debc9b1b5e2fdb7288be5179a429e406f6edc91c`.
- Base: `9ea0c9c22d0d0595a166becbba7f9e8872247c22` (current `origin/v1`).
- Parents: `869f153068812002ddce401c823bdd6c617a3a4b` and the base above.
- Scope: the current v1 merge, the retained combined source, and the corrected PR description.
- The reviewer ran no builds, tests, mutation jobs, or gates.

### Integration J3 — CLOSED in source — The head includes current v1

The reviewer verified both merge parents.
The delta from `869f1530` changes thirteen files, all under `docs/`.
Every file outside `docs/` matches the preceding combined head exactly.
The complete `docs/` tree matches current v1 exactly.
The merge therefore retains the reviewed source and carries the v1 documentation without a conflict edit.

### P5-F12 — CLOSED — The description names the final combined head

P5 updated the description and then merged current v1 for integration J3.
The reviewer read the final description and verified the current remote head.
It names `debc9b1b5e2fdb7288be5179a429e406f6edc91c` and accepted dependency `47ae53a79b95e5499b8548c456a2fbaceacba992`.
It describes both dependency merges and the current documentation merge.
It states that P5-F4/C2 closes in the merged source.
It preserves older results under their original heads and identifies the dependency Mac proof as focused evidence.
It marks the combined Linux gate pending and retains the Prior art section.
P5-F12 is CLOSED.

### Combined source and scope limits

Round 8's source conclusions remain valid because this merge changes no source.
P5-F4/C2 and the dependency findings C3 through C7 remain closed in this exact combined head.
The unchanged A10 test retains its FIFO EOF proof, production reaper ownership, derived cleanup allowance, and panic propagation.
No finding remains open within this PR's package review scope.
The separate #163 F39/G2 duty remains outside this closure.
This review does not close #164's future merge, the other P5 audit work, or P5 adoption.

The implementer supplied no execution result for this exact combined head.
The dependency's focused Mac proof does not replace the required combined Linux gate.
The gate remains the implementer's responsibility after both exact-head source reviews report CLEAN.

VERDICT: CLEAN


## PR #165 — Round 8

- Exact head: `29b37890efeffa4410d7dfc34f5c2344bfad1ba4`.
- Reviewed predecessor: `47ae53a79b95e5499b8548c456a2fbaceacba992` (CLEAN in Round 7).
- Scope: the timer comment placement in `crates/botster-core-sys/tests/common/guard_cleanup.rs`.
- The reviewer ran no builds, tests, mutation jobs, or gates.

### Timer comment placement

The complete delta moves one timer comment directly above `.recv_timeout(CLEANUP)` in the formatted method chain.
It changes no expression, timeout value, cleanup order, or test behavior.
The bounded FIFO observation retains its existing deadline.
Round 7's source conclusions remain valid for this head.
No finding remains open within this delta's package review scope.
The separate #163 guard-drop duty remains outside this closure.

### Supplied evidence and next review

The reviewer read the raw Linux log:
`~/botster-sessions/gates/botster-core-stage1-p3-guard-macos-29b37890-pool-20261008-210034-20788.log`.
Job: `jobq-botster-core-29b37890-20261008210034-3e36`.
The log identifies the exact head above and reports formatting, taint/timers, and lists PASS; exit 0 after 18 seconds.
These static checks do not establish guard test execution or a full landing gate.
The PR description attributes the previous focused Mac proof to its original `47ae53a7` head.
The combined #162 head must merge this dependency and receive a new delta review before its next authorized gate.

VERDICT: CLEAN


## PR #162 — Round 10

- Exact head: `d650e31fe0111721f2d59e3d23bfe2d3f6b29af2`.
- Parents: accepted combined head `debc9b1b5e2fdb7288be5179a429e406f6edc91c` and accepted #165 head `29b37890efeffa4410d7dfc34f5c2344bfad1ba4`.
- Current v1: `9ea0c9c22d0d0595a166becbba7f9e8872247c22`.
- Tree: `32cf311158c460726e5c7b20dbcfbda77c4bb49b`.
- Scope: the dependency merge delta and the updated PR description.
- The reviewer ran no builds, tests, mutation jobs, or gates.

### Combined source

The reviewer verified both merge parents and read the complete delta from the preceding combined head.
The delta only moves the timer comment directly above `.recv_timeout(CLEANUP)` in `guard_cleanup.rs`.
The complete changed file matches accepted #165 head `29b37890` exactly.
Every other file matches accepted combined head `debc9b1b` exactly.
No conflict edit, expression change, timeout value change, or cleanup order change occurs.
Round 9's combined source conclusions remain valid at this exact head.
P5-F4/C2 and the dependency findings C3 through C7 remain closed within this scope.
The separate #163 guard-drop duty remains outside this closure.
This review does not close #164's future merge, the other P5 audit work, or P5 adoption.

### Description and evidence

The reviewer verified the remote head and read the current PR description.
It names this combined head and the accepted dependency head.
It records the failed Linux gate under its original `debc9b1b` head and marks this head's gate pending.
It retains the A10 proof, prior review history, and Prior art section.
No description finding remains open.

The reviewer read the failed gate log:
`~/botster-sessions/gates/botster-core-stage1-p5-a10-debc9b1b-pool-20261008-205639-1469.log`.
Job: `jobq-botster-core-debc9b1b-20261008205639-b149`.
The log reports formatting and Clippy PASS, then one timer comment violation at `guard_cleanup.rs:432`.
All later gate steps report NOT RUN; exit 1 after 102 seconds.
The accepted dependency delta corrects that comment placement.
The dependency's supplied Linux static checks pass, as recorded in #165 Round 8.
Those checks do not replace the required full gate for this combined head.
No execution result was supplied for this exact combined head.
The implementer remains responsible for the authorized gate after both exact-head source reviews report CLEAN.

VERDICT: CLEAN
