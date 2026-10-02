# Stage 1 Core plan review

## Round 1 — revision 1

Plan: `47630cb77a325d12fba257220b504f46cded5816`, `docs/stage1-plan.md`.
Pin: `stage1-plan.0071b10d.md`.
Verified SHA-256: `0071b10d544cbc41a7e316a81283f7de32d63a948689dd0fd2b3952427a4212c`.

Binding source revision: botster-contracts `9666bf5cbb9e7a46cd40d98810d473a9452541e9`.
All contract citations below refer to `frozen/current/` at that revision.
All plan quotes refer to the plan commit above.

This round reviews logic only. I ran no product tests.
The checked-in ownership lists cover all 578 Core ledger ids exactly once.
The lists contain no extra ids, duplicate ids, or incorrect clause fields.
The listed contract hashes match the pinned files.
The plan correctly assigns terminal semantics to libghostty and rejects the old terminal scanner.
The old binding exposes the input encoders that section 7.1 names.

### F1 — MAJOR — Random values have no defined injected source

Status: OPEN.
Plan sections: 2.1, 2.3, 2.4, and 4.1.

Plan evidence: a machine "draws no random number"; the edge table claims to map "each" source of nondeterminism.
The table specifies no source for identity or authentication randomness.
The seeded scheduler specifies scheduling choices, not these values.

Binding evidence:
- `docs/BUILD.md`, HARD Phase 1 requirement: "Every source of nondeterminism or I/O is an injected edge".
- `core-contract-v1.17.md`, AD-6: the registry stores "a random per-worker token".
- The same file, SV-1: `CORE_SERVICE_SECRET` is "32 random bytes, hex".
- Core Amendment 5, A5-1: "the same script, seed and inputs give the same events and results".

The plan does not define how production obtains these values or how the testkit reproduces them.
Moving randomness into a driver alone does not establish deterministic testkit behavior.

Required change: define the injected source for random values and identify its consumers.
Specify an OS-backed production implementation and a deterministic testkit implementation.
Keep authentication secrets unpredictable in production.
Include randomness used by the chosen WebRTC implementation in the source audit.

### F2 — MAJOR — The testkit omits the synchronous failure mechanism

Status: OPEN.
Plan sections: 2.3 and 4.2.

Plan evidence: the fault table describes failed writes, refused spawns, broken links, and other edge failures.
Section 4.2 describes controls through injected storage, processes, and OS actions.
Neither section defines scripted refusals before admission or the allowed code table for each call.

Binding evidence: Core Amendment 5, A5-3:
> "Synchronous refusal" ... "before real Core admits anything".
> "Only a code in that call's sync column may be scripted."
> "Asynchronous failure" ... "only an edge produces it".

Edge failures after admission do not implement the required synchronous refusal mechanism.
The plan must keep these two timings separate.

Required change: specify the testkit mechanism that returns a scripted refusal before admission.
Validate each scripted code against the call's synchronous column in ER-0 and A2-1.
State that this path takes no ownership and creates no operation, event, slot, or state change.
Keep asynchronous failures on the real completion path through injected edges.
Assign implementation and proof ownership to P6, with the required P1 interface.

### F3 — MAJOR — The host wake design does not cover service lanes

Status: OPEN.
Plan sections: 2.2, 2.3, and 2.5.

Plan evidence: the wake descriptor covers "every control link" and a self-pipe.
Section 2.5 says that `pump` "drains every link non-blockingly".
The plan places service listeners in the host, but it defines no wake registration for listeners or service lane sockets.

Binding evidence:
- `core-contract-v1.17.md`, section 10: "Core (the host library) listens; the service connects".
- The same file, SV-3: inbound traffic is lossless, lanes are independent, and `ServiceReadable` has "true level semantics".
- The same file, TM-6: "Runnable work always wakes the host."

A service can connect or send a frame without producing guardian control traffic.
With the stated wake registrations, a waiting host can miss that work.
An outbound lane that becomes writable also needs a progress source.

Required change: include service listeners and lane sockets in the host readiness design.
Specify read and write interests, registration changes, and wake behavior for each lane.
Specify how full inbound queues disable reads and how `service_recv` restores progress.
Keep blocked lanes from producing a wake loop or blocking another lane.
Give the in-memory lane edge the corresponding readiness behavior after Q3 closes.

### F4 — MAJOR — A testkit-only list cannot exempt a clause from the two runs

Status: OPEN.
Plan sections: 1 and 4.2.

Plan evidence, section 4.2:
> "A control that only one harness can cause is `unsupported_control` on the other, and the id is listed in the real-only or testkit-only list with its reason."

Binding evidence:
- Core Amendment 5, A5-4: "Both run the same suite and both must pass".
- Core Amendment 5, A5-3 permits a named real-process proof for a condition that only the real OS can produce.
- `docs/foundation-design.md`, 6.1: "Only `passed` is a pass."

The contract defines the real-OS proof category. It does not define a testkit-only acceptance exemption.
The quoted rule also conflicts with section 1's statement that `unsupported_control` does not count as a pass.

Required change: remove the testkit-only acceptance exemption.
Define how both harnesses implement controls for behavior that both must prove.
Limit real-only classification to the reviewed replacement map and the contract's permitted real-OS conditions.
Require a named passing real-process proof for each such condition.
Treat every other unsupported control as an unresolved acceptance failure.

### F5 — MAJOR — The landing gate omits adopted mutation and fuzz checks

Status: OPEN.
Plan sections: 7.2 and 8.

Plan evidence: section 7.2 promises `cargo-mutants --in-diff` "at landing" and a Bolero harness per decoder.
Section 8's complete CI sequence ends after the default and slow test tiers.
The sequence contains no mutation or decoder fuzz step.

Binding evidence, `docs/BUILD.md`, Testing, adopted tools:
> "bolero, one harness per decoder; it runs as a property test on stable and as a fuzzer at landing."
> "Mutation tests at landing: cargo-mutants `--in-diff` on the changed crates".
> "A surviving mutant is a review finding."

The defined gate does not execute the checks that the plan and BUILD require at landing.

Required change: add the mutation and decoder fuzz checks to the documented landing gate.
Specify the changed-crate scope, base revision, execution limits, and recorded results.
State how the reviewer closes a surviving mutant before the lead merges the exact head.
Run heavy local checks through the existing exclusive `botsterq` gate while the interim rule applies.

### F6 — MINOR — The pinned runner does not read the proposed pending file

Status: OPEN.
Plan sections: 4.3, 5, and 8.

Plan evidence: section 8 says that `conformance/core-pending.txt` lists ids not yet expected to pass and "The suite marks them ignored."
Section 5 calls the pinned runner's existing macro without a pending-file integration point.

Source evidence at botster-contracts `9666bf5cbb9e7a46cd40d98810d473a9452541e9`:
- `crates/botster-core-conformance/build.rs` collects ids from the dependency's transcript directory.
- `crates/botster-conformance/src/testgen.rs` generates `#[test]` functions that call `run_one_test`.
- `crates/botster-conformance/src/run.rs` selects tests with `Selection::from_env()` and panics when `!outcome.is_pass()`.
- `crates/botster-conformance/src/seeds.rs` supports id, clause, and seed selection. It has no pending-file selection.

The current runner neither discovers missing ledger ids nor marks the consumer's pending ids ignored.
The plan needs an explicit implementation step for this interim gate.

Required change: define the consumer-side generation or runner change that applies the pending list.
Assign that step to a package and milestone.
Report pending ids separately from passing ids.
Keep the empty-list condition as a Stage 1 acceptance requirement.
If the runner must change, add that change to the Stage 0 needs table.

### F7 — MINOR — Q1 overstates the effect of a Ghostty pin change

Status: OPEN.
Plan section: 9, Q1.

Plan evidence:
> "A pin change also changes `terminfo_source` (A2-8: \"a contract-tag change\")".

Binding evidence, `core-contract-v1.17-amendment-2-candidate4.md`, A2-8:
> "A change of the pinned emulator that changes the entry changes `terminfo_source` and is a contract-tag change."

The clause applies when the emulator change changes the terminfo entry.
A callback patch does not necessarily change that entry.

Required change: preserve the clause's condition in Q1.
Require an audit of the entry after a pin change.
Require a changed `terminfo_source` and contract tag when the entry changes.

### F8 — MAJOR — File operations need a nonblocking completion design

Status: OPEN.
Plan sections: 2.2, 2.3, and 6.1, P4b.

Plan evidence: the worker uses a single `mio` loop; the real `FileSink` uses "real files".
The testkit `FileSink` also uses real files in its temporary root.
The plan defines no separate execution path or completion input for file operations.

Binding evidence, `core-contract-v1.17.md`, DP-5b:
> "file I/O never blocks the PTY reader or another route".

Ordinary file creation and writes can block the calling thread.
Putting those calls behind a trait does not prevent them from blocking the worker loop.
Using real file completion timing in the testkit also needs deterministic scheduling.

Required change: define how the file edge executes blocking operations outside the worker's readiness loop.
Return completion inputs to the same Worker machine through a bounded interface.
Specify backpressure, completion order, abort behavior, and cleanup.
Let the testkit script completion timing at that interface.
Keep the worker logic common to both runs.

VERDICT: NOT CLEAN (8 open)

## Round 2 — revision 2

Plan: `cbcf022deba7303f44dbc4984da32c02a9819f07`, `docs/stage1-plan.md`.
Pin: `stage1-plan.29cb43ee.md`.
Verified SHA-256: `29cb43ee08de062612b039963e918dcc31a45c64024229622b1697bcd069e10c`.
The binding source revision remains `9666bf5cbb9e7a46cd40d98810d473a9452541e9`.

I reviewed the complete delta and all eight open findings.
The ownership generator and lists did not change.
I ran no product tests.

### Closed findings

- F1: CLOSED. Sections 2.3a and 2.3c define the entropy source, its consumers, deterministic test values, and the WebRTC audit.
- F2: CLOSED. Section 4.2a separates refusals before admission from edge failures after admission. Both harnesses use the refusal layer.
- F4: CLOSED. Section 4.2b removes the testkit-only exemption. Other non-passing results remain acceptance failures.
- F7: CLOSED. Q1 preserves A2-8's condition and requires an audit of the terminfo entry.

### F3 — MAJOR — Read interest must also follow epoch and queue state

Status: OPEN, narrowed.
Plan section: 2.5.

The revision registers service listeners and sockets. This closes the missing-registration part of F3.
The new registration rules still permit reads when the host cannot consume them.

Plan evidence:
> An authenticated lane socket has read interest "only while that lane's inbound queue has room for one more frame".
> A worker or guardian control link has read interest "always, until the link ends".
> "Work that is parked on mandatory-queue room (EV-5b) has no registration effect."

Binding evidence, `core-contract-v1.17.md` at the binding revision:
- SV-6: "Until then no payload frame is read or delivered." This condition holds until every lane authenticates and Core commits the epoch.
- TM-6: work blocked on mandatory-queue room "is not runnable" and "keeps `more` false".
- TM-6: "A due stall or state transition that is parked never makes `pump` return at once in a loop."

An authenticated lane can still belong to an uncommitted epoch.
Queue room alone does not permit Core to read that lane's payload.
An always-readable control link also needs a bounded consumption rule when mandatory events cannot progress.
Leaving blocked bytes readable can repeatedly wake a host that cannot consume them.

Required change: enable payload reads only after Core commits the lane's epoch.
Disable read interest on staged lanes after Core reads their preambles.
Restore read interest when the epoch commits and inbound capacity permits a read.
Define bounded control receive storage and disable read interest when Core cannot consume more input.
Restore that interest when mandatory-queue room or another required condition permits progress.
Apply the same readiness rules in the testkit.

### F5 — MINOR — The fuzz command omits the required nightly toolchain

Status: OPEN, narrowed from MAJOR.
Plan section: 8, step 9.

The revision adds mutation and fuzz checks to the gate. This closes their omission.
The fuzz command does not match the cited tooling source.

Plan evidence:
> "cargo bolero test … -T 60s", "as tooling.md states".

Source evidence, `docs/tooling.md` at the binding revision, Fuzzing row:
> "cargo +nightly bolero test -T 60s".

The workspace pins stable Rust `1.97.0` in section 0.
The plan gives no nightly setup for the landing fuzzer.

Required change: specify the nightly toolchain used by the Bolero landing fuzzer.
Use that toolchain explicitly in the gate command.
Keep the stable property-test path in the default tier.
Record the nightly pin when P0 fixes the gate tool versions.

### F6 — MINOR — The pending-list check rejects its first commit

Status: OPEN, narrowed.
Plan section: 5.

The public runner functions support the proposed consumer harness.
This closes the missing integration mechanism.
The new monotonic check cannot bootstrap the pending file on the empty branch.

Plan evidence:
> "cargo xtask ci fails if it gains an id that the `origin/v1` head did not list."
> "P0 builds this harness and the two files at M0, with every id pending."

Source evidence:
- The plan's section 0 pins `origin/v1` to `d91495eb8b6e4c730177c1d440d2d06ce762caa5`.
- That commit has no `conformance/core-pending.txt`.
- `docs/BUILD.md` at the binding revision requires an empty `v1` branch with "no old files".

P0 necessarily adds pending ids that the initial `origin/v1` head did not list.
The stated check therefore rejects P0's gate.

Required change: define a one-time initialization rule when the base contains no pending file.
Validate that initial list against the complete pinned Core ledger.
Apply the shrink-only check after initialization.
Keep missing-transcript ids visible and require zero pending ids of either kind at Stage 1 acceptance.

### F8 — MAJOR — Remove must wait for file deletion completions

Status: OPEN, narrowed.
Plan section: 2.3b.

The revision defines the file request interface, file thread, bounded writes, and scheduled testkit completions.
This closes the blocking-I/O part of F8.
The asynchronous cleanup rule still specifies request order instead of completion order.

Plan evidence:
> "the worker requests `Delete` for every file that its routes wrote, before it ends."

Binding evidence, `core-contract-v1.17.md` at the binding revision:
- LC-7, step 3: "the files that the session's routes uploaded (DP-5b) are deleted, and the worker ends".
- LC-7: "`Completed{Remove}` is posted only after step 5".
- DP-5b: files "are deleted at `Remove` (LC-7) at the latest".

A queued deletion request does not establish that the file was deleted.
The worker must retain the file edge until deletion completes.
The host must not free the durable row or id before the required cleanup completes.

Required change: wait for the required `Deleted` completions before the worker ends.
Keep deletion progress in the teardown state machine.
Do not advance LC-7 steps 4 and 5 until step 3 completes.
State how pending creates and writes finish or cancel before deletion.
Treat a deletion failure as unresolved teardown work or ask the steward if the required outcome is not specified.
Do not report successful removal while an uploaded file remains.

VERDICT: NOT CLEAN (4 open)

## Round 3 — revision 3

Plan: `6c82fb316c213bba56191f3f0cb984763564b922`, `docs/stage1-plan.md`.
Pin: `stage1-plan.ad4d340d.md`.
Verified SHA-256: `ad4d340df5395c61b0e15671c0a407c4cecb0c8675fde44cf5258122b286bf6f`.
The binding source revision remains `9666bf5cbb9e7a46cd40d98810d473a9452541e9`.

I reviewed the complete delta and every remaining finding.
I ran no product tests.

### Closed findings

- F3: CLOSED. Section 2.5 gates lane reads on epoch commit and capacity. It parks control reads when mandatory events cannot progress.
- F5: CLOSED. Section 8 uses a separately pinned nightly toolchain for landing fuzz checks and stable Rust for property tests.
- F6: CLOSED. Section 5 defines initialization, later shrink-only checks, and zero pending ids of both kinds at acceptance.
- F1, F2, F4, and F7 remain CLOSED.

### F8 — MAJOR — File ownership must survive worker loss

Status: OPEN, narrowed.
Plan sections: 2.3b and 9, Q5.

The normal teardown now waits for file completions before the worker ends or the host frees the row.
This closes the request-versus-completion defect.
Asking the steward about a failed deletion is appropriate because A2-1 supplies no dedicated file-deletion failure outcome.
The unreachable-worker case has a different cause: the plan keeps the required ownership information only in the worker.

Plan evidence:
> "Where no worker is reachable, the host cannot know the file names; Q5 includes this case."
> Q5: "only the worker knows the file names".

Binding evidence at the binding revision:
- `core-contract-v1.17.md`, LC-7: `Remove` "applies to `Created`, `Exited` and `Lost` sessions".
- LC-7, step 3 requires deletion of uploaded files before the durable row and id are freed.
- DP-5b requires deletion "at `Remove` (LC-7) at the latest".
- `core-contract-v1.17-amendment-2-candidate4.md`, A2-1 admits `Remove` in `Lost` and returns `()` after LC-7 step 5.

These clauses do not make file cleanup conditional on worker reachability.
The contract does not require the worker to be the only keeper of file ownership information.
That restriction comes from the plan's design.
A reachable-worker requirement would leave required removal behavior unavailable after worker loss.

Required change: define file ownership information that survives loss of the worker and restart of the host.
Let teardown identify and delete this session's uploaded and partial files without a reachable worker.
Protect files that the session does not own.
Define the order that records ownership before an upload can leave a file requiring later cleanup.
Assign the metadata and cleanup interfaces to P4b, P1, and P5 as needed.
Keep Q5 for actual deletion failure semantics.
Do not classify the ordinary `Lost` removal path as a missing contract outcome caused by worker-local metadata.

VERDICT: NOT CLEAN (1 open)

## Round 4 — revision 4

Plan: `feb537652fa20b8e9883300791e535f1d4707c37`, `docs/stage1-plan.md`.
Pin: `stage1-plan.5647f413.md`.
Verified SHA-256: `5647f413c5939fcc20d4050466fe4b274eeade6478e2f6dea3a16073c2133553`.
The accepted contract set remains pinned at `9666bf5cbb9e7a46cd40d98810d473a9452541e9`.

Additional design input: Core Amendment 6 candidate 1 at botster-contracts `37f083b40d47f13158bdcdfb47f39ef33137d1f5`.
Path: `amendments/core-contract-v1.17-amendment-6-candidate1.md`.
Verified SHA-256: `8332c662eba22d585b11e919adbbb2587a9f9bd57f9d8f3012400c1f55898289`.
This candidate is not yet accepted.

I reviewed the complete delta, the amendment candidate, and F8.
The changed ownership rules still reproduce the current 578-id allocation without a mismatch.
I ran no product tests.

### F8 — CLOSED through the amendment path

Section 2.3b now matches A6-3's proposed cleanup behavior and assigns each part to a package.
The worker waits for deletion results before it ends.
The host advances teardown only after it knows the cleanup result.
`RemoveReport` exposes deletion failure or worker loss instead of silently claiming that files were deleted.

The lead filed the required contract change with the steward.
This resolves the design finding through the amendment path that BUILD permits.
It does not approve the amendment or waive its review and manifest requirements.
The plan identifies those requirements in section 0.
F1 through F7 remain CLOSED.

### F9 — MAJOR — The gate does not define the new deferred category

Status: OPEN.
Plan sections: 0, 1, 4.2b, and 5.

Plan evidence:
- Section 1 item 5 records N−1 worker ids as deferred, "neither passed nor pending".
- Section 5's trial rules support runnable transcripts, pending ids, and missing transcripts. They do not classify deferred ids.
- Section 5 still permits a package to remove an id from the pending file only when it passes on both harnesses.
- Section 4.2b still makes every other non-pass an acceptance failure.
- Section 0 keeps "its ids" pending until A6's ACK and manifest entry, but does not explicitly cover existing AD-4 ids that A6 proposes to defer.

Source evidence:
- A6-2 candidate 1 requires each deferred id to be recorded with A6-2 as its authority. It is "neither passed nor pending".
- `crates/botster-conformance/src/result.rs` at the accepted contract revision defines no deferred outcome.
- `docs/BUILD.md` at that revision requires contract amendments to land through the steward and Amendment reviewer.
- AD-4 in the accepted `core-contract-v1.17.md` still requires the N−1 adoption proof.

The custom consumer harness must implement this new category.
Without that step, the ids either remain pending, fail through `worker(Previous)`, or disappear without a recorded classification.
Existing ids also must not gain the proposed exemption before A6 is accepted.

Required change: define a checked-in list of the exact deferred ids with A6-2 citations.
Have the consumer harness report deferred ids separately from passed, failed, and pending ids.
Define the authorized move from pending to deferred after A6's ACK, manifest entry, and contracts pin update.
Make the gate validate each deferred id against that accepted list and the running worker protocol.
Keep existing AD-4 obligations pending before A6 is accepted.
At protocol 2, make deferral invalid and require the pinned protocol-1 worker proofs.
Update sections 1, 4.2b, and 5 so that their acceptance rules agree.

VERDICT: NOT CLEAN (1 open)

## Round 5 — revision 5

Plan: `fa3b3abf4dfcdb8c76f3de086b5fc3203fc733d9`, `docs/stage1-plan.md`.
Pin: `stage1-plan.1be2ed10.md`.
Verified SHA-256: `1be2ed103a31ddc33fb3fbd4606b3d4646390b204e0501952afec183b05d8d2e`.
The accepted contract set remains pinned at `9666bf5cbb9e7a46cd40d98810d473a9452541e9`.

Additional design input: Core Amendment 6 candidate 2 at botster-contracts `0aa03e569d05481edcb4a8c297c9fc1a063b5fdb`.
Path: `amendments/core-contract-v1.17-amendment-6-candidate2.md`.
Verified SHA-256: `b1e674cc0100823bf0acdb7ef0b4bf8bf8cbe9d729b1c80f6bb4d7ad935676d3`.
The candidate is not yet accepted.

I reviewed the complete delta and F9.
The cleanup design matches candidate 2's complete trusted result, exact failure paths, and unknown-outcome rules.
F1 through F8 remain CLOSED.
I ran no product tests.

### F9 — MINOR — One sentence still ends both deferrals at protocol 2

Status: OPEN, narrowed from MAJOR.
Plan section: 5, deferred-list validation rule 4.

The revision defines the deferred list, report category, accepted-manifest condition, and pin update.
Sections 1 and 4.2b now recognize that category.
These changes close the gate-design parts of F9.

Plan evidence, rule 4:
> "so at protocol 2 the deferral ends and the tests against the pinned protocol-1 worker binary must pass."

Source evidence, A6-2 candidate 2 at `0aa03e569d05481edcb4a8c297c9fc1a063b5fdb`:
> "A release whose new protocol adds no worker feature does not end the deferral, because the test has no subject."

The start conditions differ.
Protocol 2 ends the previous-worker adoption deferral.
Protocol 2 ends the missing-capability deferral only if that protocol adds a feature absent from the pinned previous worker.
The quoted plan sentence contradicts its preceding checks and section 1.

Required change: state each start condition separately in the final sentence of rule 4.
Do not require the missing-capability test at protocol 2 if the pinned previous worker lacks no current feature.

### F10 — LOW — Revision history names the wrong amendment candidate

Status: OPEN.
Plan section: 12, revision 4 row.

Plan evidence:
> "the steward's answer (Core A6 candidate 2, `37f083b`)".

Source evidence:
- botster-contracts `37f083b40d47f13158bdcdfb47f39ef33137d1f5` adds `amendments/core-contract-v1.17-amendment-6-candidate1.md`.
- Candidate 2 at `0aa03e569d05481edcb4a8c297c9fc1a063b5fdb` states: "Supersedes candidate 1" and names commit `37f083b`.

Required change: restore candidate 1 in the revision 4 history row.
Keep candidate 2 and `0aa03e5` in the revision 5 row.

VERDICT: NOT CLEAN (2 open)

## Round 6 — revision 6

Plan: `c8ad7ec52232ea29b16ca21c49d8b200adc9c52e`, `docs/stage1-plan.md`.
Pin: `stage1-plan.1327dca0.md`.
Verified SHA-256: `1327dca0abd84739b7a6536dfb5f20c2770db011c71c72ef46b847773ae50fe5`.

I reviewed the complete delta and both remaining findings.
I ran no product tests.

- F9: CLOSED. Section 5 rule 4 now states the two start conditions separately, as A6-2 candidate 2 requires.
- F10: CLOSED. The revision 4 history row correctly identifies candidate 1 at `37f083b`.
- F1 through F8 remain CLOSED.

All ten findings are closed. This verdict covers the plan at the commit above.
It does not approve Core Amendment 6, product code, or Stage 1 acceptance.
The plan keeps A6 changes out of acceptance until its ACK, manifest entry, and contracts pin update.
A changed amendment candidate requires a plan revision and review.

VERDICT: CLEAN

## Round 7 — revision 7

Plan: `3281c4e413c9709767c7de53b7c0e003ff3ffbae`, `docs/stage1-plan.md`.
Pin: `stage1-plan.7f7f9262.md`.
Verified SHA-256: `7f7f9262c1eeb33280b8da4d633d96ce4a53536b01b1b6d4927f9ab8500f77c5`.
Contracts pin: `2f2996ef0f016a1fefc6879e74deaef033383b66`, manifest final13.

I reviewed the delta against final13.
The accepted A6 candidate 3 has the claimed hash and Amendment reviewer ACCEPT.
The withheld-link design preserves `WorkerUnreachable` while the worker remains alive.
All 591 Core ids have exactly one owner.
The generator reproduces every list, including its pending fields.
The ledger has 459 pending Core transcripts, as the plan states.
F1 through F10 remain CLOSED.
I ran no product tests.

### F11 — MAJOR — The new BUILD pin requires local concurrency limits

Status: OPEN.
Plan sections: 0 and 8.

Plan evidence, section 0:
> "BUILD.md and the rulings are unchanged between the two."

The statement about BUILD.md is false.
Section 8 also gives no explicit concurrency limits for local gates or builds.

Binding evidence, `docs/BUILD.md` at `2f2996ef0f016a1fefc6879e74deaef033383b66`, Resources:
> "every gate and local build caps its parallelism, with `CARGO_BUILD_JOBS=4` and nextest `--test-threads 4` (or `NEXTEST_TEST_THREADS=4`)."

The diff from the old contracts pin adds this requirement.
An exclusive `botsterq` gate prevents competing gates. It does not limit concurrency inside the gate.

Required change: correct the section 0 statement about BUILD.md.
Apply `CARGO_BUILD_JOBS=4` to every interim local gate and local build.
Apply the required four-thread limit to every nextest invocation in those gates.
Make nested build and test commands inherit the limits, including mutation commands.
Record these limits in section 8's CI and interim-gate rules.

VERDICT: NOT CLEAN (1 open)

## Round 8 — revision 8

Plan: `8c491c096a127405d8ff9438a39a0f4218a1fd41`, `docs/stage1-plan.md`.
Pin: `stage1-plan.0ad8f09e.md`.
Verified SHA-256: `0ad8f09e63511b43123030bf3bbbd8aa8a62b0492c542a0a48ed5a0656d49929`.
Contracts pin: `2f2996ef0f016a1fefc6879e74deaef033383b66`, manifest final13.

I reviewed the complete delta and F11.
The section 0 history correction is accurate.
The steward rulings and tooling document did not change between the two contract pins.
F1 through F10 remain CLOSED.
I ran no product tests.

### F11 — MINOR — Set the limits before Cargo starts the xtask

Status: OPEN, narrowed from MAJOR.
Plan section: 8.

The revision limits child builds, nextest, mutation work, and fuzz work.
This closes the missing child-command limits.
The gate launcher still needs the limits before it starts Cargo.

Plan evidence:
> "cargo xtask ci sets `CARGO_BUILD_JOBS=4` and `NEXTEST_TEST_THREADS=4` in its own environment before it starts any step".

Source evidence at the contracts pin:
- `.cargo/config.toml` defines `xtask = "run --quiet --package xtask --"`.
- `docs/BUILD.md`, Resources, requires the limit on "every gate and local build".

With this Cargo alias, Cargo can build the xtask executable before the executable sets its environment.
An environment change inside the executable cannot limit that earlier build.

Required change: set both variables in the gate launch environment before invoking `cargo xtask ci`.
For example, the command run through `botsterq` can start with `env CARGO_BUILD_JOBS=4 NEXTEST_TEST_THREADS=4 cargo xtask ci`.
Keep the xtask's child-command enforcement and checks.
Apply the same launch rule to focused commands that build an xtask.

VERDICT: NOT CLEAN (1 open)

## Round 9 — revision 9

Plan: `194f7437e11debc85a9af88a6b7ef27b967f86f6`, `docs/stage1-plan.md`.
Pin: `stage1-plan.a24efe7e.md`.
Verified SHA-256: `a24efe7e3f5a18631ab8c6032ce15dbf39c48e70d33a2cd14b7269cef3d394dc`.
Contracts pin: `2f2996ef0f016a1fefc6879e74deaef033383b66`, manifest final13.

I reviewed the complete delta and F11.
Section 8 now sets both concurrency limits in the launch environment before Cargo builds the xtask.
The xtask retains enforcement for its child commands.

F11 is CLOSED. F1 through F10 remain CLOSED.
All eleven findings are closed.
This verdict covers the plan at the commit above.
It does not approve product code or establish Stage 1 acceptance.
I ran no product tests.

VERDICT: CLEAN

## Round 10 — revision 10

Plan: `e70814647686347dafc1f3e32e0a00de21595952`, `docs/stage1-plan.md`.
Pin: `stage1-plan.f7a8aeab.md`.
Verified SHA-256: `f7a8aeab34e3ee4dcfec5abafc33bfb0942bcd59256327bbea6a77c5dee5d7bc`.
Contracts tag: `contracts-v0.1.1`, commit `366bca41da0a6de69cc1ea13b17c773cdfdb75b6`, manifest final14.

I reviewed the delta against the new contracts tag.
The erratum hash, ACCEPT, manifest tag, and release tag match the plan.
The worker rule matches E2-3's final-state comparison and admission between model steps.
The R-13 summary and new spawn rule match their pinned sources.
The probe binary depends on `botster-probe-script`, which has no fake dependency.
The ownership generator reproduces all lists, with 599 unique ids and 145 ids assigned to P3.
F1 through F11 remain CLOSED.
I ran no product tests.

### F12 — LOW — R2 still says that no release tag exists

Status: OPEN.
Plan section: 10, R2.

Plan evidence:
> "no release tag exists."

Source evidence:
- `contracts-v0.1.1` resolves to `366bca41da0a6de69cc1ea13b17c773cdfdb75b6`.
- Sections 0 and 4.3 correctly pin this release tag and mark the tag requirement DONE.

R2 presents the absence of a release tag as a current risk.
That statement conflicts with the completed dependency change in this revision.

Required change: remove the claim that no release tag exists from R2.
Keep the remaining transcript and replacement-map risks.

VERDICT: NOT CLEAN (1 open)

## Round 11 — revision 11

Plan: `f3ccca5f282ee027015452cecf78b4c25461d0d3`, `docs/stage1-plan.md`.
Pin: `stage1-plan.555bc433.md`.
Verified SHA-256: `555bc4337fe72e9fad833fe330d43e8147a39569cb14291734f34847f56596d7`.
Contracts tag: `contracts-v0.1.1`, commit `366bca41da0a6de69cc1ea13b17c773cdfdb75b6`, manifest final14.

I reviewed the complete delta and F12.
R2 now states that the release tags exist and retains the unfinished Foundation work.
F12 is CLOSED. F1 through F11 remain CLOSED.
All twelve findings are closed.
This verdict covers the plan at the commit above.
It does not approve product code or establish Stage 1 acceptance.
I ran no product tests.

VERDICT: CLEAN

## Round 12 — revision 12

Plan: `0a8c80351746d01aaa79b994570c9506312766e9`, `docs/stage1-plan.md`.
Pin: `stage1-plan.00f63f57.md`.
Verified SHA-256: `00f63f578ceac79d952b72ccb17a8d263f297a09331d7fb6dcc911db1afce4d2`.
Contracts tag: `contracts-v0.1.3`, commit `943d5b98df725c962bf67251e1834e606ef929c2`, manifest final16.

I reviewed the complete delta against the new contracts tag.
The plan matches its pin.
The A7 and A8 hashes and manifest tags match their pinned sources.
The plan states the A7 refusal rule and the SCTP chunk exception correctly.
The A8 summary preserves the reservation until polling and the qualified snapshot refusal.
The ownership generator reproduces every list with 622 unique ids, correct clauses, and correct transcript status.
P1 owns 117 ids, P2 owns 6 ids, and P4a owns 148 ids.
The probe implements ordered steps, `ignore_sigterm`, and a direct `signal_self` as section 4.3 states.
The BUILD reporting summary and R-14 summary match their pinned sources.
Section 8 preserves the concurrency caps and adds the exclusive heavy-job rule before the gate starts.
F1 through F12 remain CLOSED. I found no new finding.
This verdict covers the plan at the commit above.
It does not approve product code or establish Stage 1 acceptance.
I ran no product tests.

VERDICT: CLEAN

## Round 13 — revision 13

Plan: `bfdc82059b6e00f33ef36765a33b71a579ecc788`, `docs/stage1-plan.md`.
Pin: `stage1-plan.dcee98cb.md`.
Verified SHA-256: `dcee98cb434017049a4869ceb8aa33acc40d97298323f37809ac0c1b7fef6d7f`.
Contracts tag: `contracts-v0.1.6`, commit `caa029cfcc9c0bfd1f59a05d35d7c110ea99f3e0`, manifest final19.

I reviewed the complete delta against the new contracts tag.
The plan matches its pin. The three new contract hashes and manifest tags match their sources.
The A9, A10, E3-1, and R-15 through R-17 summaries match the pinned text.
The ownership generator reproduces all lists with 635 unique active ids and two withdrawn ids excluded.
P1 owns 124 ids, P4a owns 151 ids, and P5 owns 54 ids.
F1 through F12 remain CLOSED. I ran no product tests.

### F13 — MINOR — distinguish deferred ids from a not-applicable case

Status: OPEN.
Plan section: 5, status-file source and report.

Plan evidence at the plan commit above:
> "lists exactly the Core entries of `deferred.txt` (rules 1 to 4 below still apply)"

Source evidence at the contracts commit above, `conformance/deferred.txt`:
> "not-applicable conf::dp_12_unreachable_or_n_minus_1_worker_leaves_focused_unknown"
> "the id stays active and its unreachable-worker case runs now"

The file contains two deferred Core ids and one not-applicable case of a third active Core id.
The new exact-match instruction includes all three entries, while rule 3 permits only the two deferred ids.
Section 1 correctly requires the third id's unreachable-worker case to pass now.
The report also still says "four counts" immediately before adding a fifth category.

Required change: match `core-deferred.toml` only against the whole-id deferrals in the contracts file.
Validate and report the not-applicable case separately, while the active id still runs.
Update the report description to include the withdrawn category.

### F14 — MAJOR — gate checks still use a mutable base reference

Status: OPEN.
Plan sections: 5, pending-list comparison; 8, mutation step 8.

Plan evidence at the plan commit above:
> "the file gains an id that the `origin/v1` head did not list"
> "`git diff origin/v1...HEAD > target/landing.diff`"

Source evidence at the contracts commit above, `docs/BUILD.md`, "The same gate on either machine":
> "`BOTSTER_CI_BASE_REF` set to the base commit recorded when the gate starts"

Source evidence at the same commit, `ci/remote/botster-gate`, Mac path:
> "a detached worktree of the same repository"
> "BOTSTER_CI_BASE_REF=\"$base_sha\""

Section 8 now records the captured base, but the two check instructions still read `origin/v1`.
A detached Mac gate tree shares repository references with the other worktrees.
A fetch can move `origin/v1` while the gate waits or runs.
The pending check and mutation scope can then use a different base from the gate log.

Required change: use `BOTSTER_CI_BASE_REF` for the pending-list comparison and mutation diff inside the gate.
Resolve and record a base once for focused runs outside the gate.
Do not resolve the moving branch reference separately for each check.

### F15 — MINOR — the fuzz step still installs its toolchain inside the offline gate

Status: OPEN.
Plan section: 8, decoder fuzzing step 9.

Plan evidence at the plan commit above:
> "a nightly that the gate installs with `rustup toolchain install`"

Source evidence at the contracts commit above, `docs/BUILD.md`, Linux gate:
> "The image holds the toolchains of `rust-toolchain.toml`, the pinned nightly"
> "The gate container then runs with no network and no token."

The new Linux gate has no network, but step 9 still instructs the gate to install a nightly.
The image must contain that exact toolchain before the gate command starts.
Section 8 sends build-time fetches to the infrastructure path, but does not update this installation instruction.

Required change: provision the pinned nightly in the gate image before the offline gate starts.
Make step 9 verify that the toolchain exists and run it without installation or update requests.
State how the Mac gate receives the same pinned nightly before its gate starts.

VERDICT: NOT CLEAN (3 open)

## Round 14 — revision 14

Plan: `be14eb4c75681b7dda28875776d9d12e13cd5fd7`, `docs/stage1-plan.md`.
Pin: `stage1-plan.834f6cef.md`.
Verified SHA-256: `834f6cefe466b3123bae167fae1eb5902e529d0b317f07167c4cca86704ff3a9`.
Contracts tag: `contracts-v0.1.6`, commit `caa029cfcc9c0bfd1f59a05d35d7c110ea99f3e0`, manifest final19.

I reviewed the complete delta and F13 through F15.
The plan matches its pin.
F13 is CLOSED: only whole-id deferrals enter `core-deferred.toml`; the active DP-12 id runs and its excluded case is reported separately.
The report now has five counts.
F14 is CLOSED: both checks use the recorded base inside the gate, with one recorded resolution outside the gate.
F15 is CLOSED: the nightly is provisioned before the gate; the fuzz step verifies it without installation or update requests.
The pinned contracts image provisions `nightly-2026-09-30`, which matches the revised plan.
F1 through F12 remain CLOSED. All fifteen findings are closed.
This verdict covers the plan at the commit above.
It does not approve product code or establish Stage 1 acceptance.
The lead has assigned the remaining code changes to P6; this verdict does not verify those changes.
I ran no product tests.

VERDICT: CLEAN

## Round 15 — revision 15

Plan: `3cb6ec9c0aa9a21ddfdd8459c2756c9e368db7e9`, `docs/stage1-plan.md`.
Pin: `stage1-plan.c43693ff.md`.
Verified SHA-256: `c43693fffde49ea2eaaac2e9eca9c5133d380376b171f1af478423fcf6d86649`.
Contracts tag: `contracts-v0.1.7`, commit `f14c89510323fd3956219212a4dfffbf7d09f8ae`, manifest final21.

I reviewed the complete delta against the new contracts tag.
The plan matches its pin. The A11 and A12 hashes and manifest tags match their sources.
The A11 summary preserves rejection of frames from an unauthenticated impostor.
The A12 summary distinguishes a held report from cleanup interrupted before its result exists.
A12-1a loses the held report by breaking the link, as its pinned clause requires.
The R-18 summary matches the ruling. BUILD.md and tooling.md have no changes between the two contracts tags.
The ownership generator reproduces all lists with 638 unique active ids and two withdrawn ids excluded.
P4b owns 65 ids, including both new cleanup ids. P5 owns 55 ids, including the new impostor-frame id.
F1 through F15 remain CLOSED. I found no new finding.
This verdict covers the plan at the commit above.
It does not approve product code or establish Stage 1 acceptance.
I ran no product tests.

VERDICT: CLEAN

## Round 16 — revision 16

Plan: `0ba74bdd7767d53767d0baa33fc3b1a882c5f423`, `docs/stage1-plan.md`.
Pin: `stage1-plan.a7c41bc1.md`.
Verified SHA-256: `a7c41bc1cea55579526c0b4d5f7c5659c971155adba6352972d7331266dda655`.
Contracts tag: `contracts-v0.1.8`, commit `f1e6a76330c4620785b19ba112c181d906bcc449`, manifest final22.

I reviewed the complete delta against the new contracts tag.
The plan matches its pin. The erratum hash and manifest tag match their sources.
The E4-1 summary matches the client-frame and fallback-byte reservation rules.
BUILD.md, the steward rulings, and tooling.md have no changes between the two contracts tags.
The ownership generator reproduces all lists with 641 unique active ids and two withdrawn ids excluded.
P4b owns 68 ids, including all three E4-1 ids.
F1 through F15 remain CLOSED. I ran no product tests.

### F16 — LOW — the section 6.1 total retains the previous count

Status: OPEN.
Plan section: 6.1, total after the package table.

Plan evidence at the plan commit above:
> "Total: 638 active ids at the pinned tag"

Source evidence at the contracts commit above, `conformance/ledger.json` and `conformance/withdrawn.txt`:
The ledger has 643 unique Core ids. A9-2 withdraws two, so 641 are active.
The checked-in ownership lists also contain 641 unique active ids.
Section 1, R4, and revision 16's history correctly state 641.

Required change: change the section 6.1 total from 638 to 641.

VERDICT: NOT CLEAN (1 open)

## Round 17 — revision 17

Plan: `c59757ed040e06cd84b79fd0a701dc6183182bba`, `docs/stage1-plan.md`.
Pin: `stage1-plan.2b03dc1b.md`.
Verified SHA-256: `2b03dc1b68fd3da521ba2bcc9f5d4ab18380d35e784774abcfddabd26d810946`.
Contracts tag: `contracts-v0.1.8`, commit `f1e6a76330c4620785b19ba112c181d906bcc449`, manifest final22.

I reviewed the complete delta and F16.
The plan matches its pin. Section 6.1 now states 641 active ids.
F16 is CLOSED. F1 through F15 remain CLOSED.
All sixteen findings are closed. I found no new finding.
This verdict covers the plan at the commit above.
It does not approve product code or establish Stage 1 acceptance.
I ran no product tests.

VERDICT: CLEAN

## Round 18 — revision 18

Plan: `213b65355629a56bf5f9086174a2b70435a4773b`, `docs/stage1-plan.md`.
Pin: `stage1-plan.a16bee0c.md`.
Verified SHA-256: `a16bee0cb23edd7d0fe838fb2a1b226ee719dcaa4b137d875360ba8072e30613`.
Contracts tag: `contracts-v0.1.9`, commit `7f72acf8427ad7bf414db42d2360d5dccc7b3e13`, manifest final22.
Ghostty fork: `85a8d8eb197c5752887c017c9a3faa6f1dc1969b`, on upstream `83edd491e3024ae5e50393d62877b8897da1cccd`.

I reviewed the complete plan delta and the pinned audit and fork review.
The plan matches its pin. The fork contains 17 commits above the named upstream base.
The P2 verdict at `1fb4c3e` reports CLEAN for patches 0 through 8 and audit `52b3d86` at the named fork head.
That verdict excludes the Rust binding, final pin change, and full P2 package.
The terminfo entry and source encoder are identical at the old pin, upstream base, and new fork head.
The fork retains Zig 0.16.0. The audit records the seven-package offline prerequisite.
The contracts crate returns the refused transport in `AttachRefused`, as R-19 requires.
The R-19 and R-20 summaries match their sources. BUILD.md and tooling.md are unchanged between the contracts tags.
The ownership rules still cover all 641 active ids exactly once.
F1 through F16 remain CLOSED. I ran no product tests.

### F17 — MAJOR — the full-chunk model step conflicts with the query API

Status: OPEN.
Plan section: 2.4, terminal model rule.

Plan evidence at the plan commit above:
> "the worker hands each PTY chunk to libghostty in one `vt_write`"
> "never mid-chunk"

Source evidence at the new Ghostty fork head, `include/ghostty/vt/terminal.h`, query-write API:
> "Nothing after the query sequence is processed"
> "write the rest of the data with another call"
> "The query bytes are kept only by this function: ghostty_terminal_vt_write() still reports the query but without its bytes."

Source evidence at audit `52b3d86`, EV-8c and EV-8g row:
> "The worker then splits the chunk at that byte."

Section 0 adopts `vt_write_until_query`, but section 2.4 still requires the full chunk to enter a single ordinary write.
That ordinary write does not provide the request bytes that EV-8 needs.
The query-aware function can return before the chunk ends, so a PTY chunk is no longer necessarily one model step.
E2-3 requires input admission between model steps and final-mode comparison after each step; it does not require a full PTY chunk per step.

Required change: define a model step using the bytes that the query-aware call actually consumes.
Retain the unconsumed suffix and honor query-capacity backpressure before the next step.
Preserve E2-3's final-mode comparison and admit input only between completed model calls, never inside a callback.
Remove the incompatible full-chunk requirement.

### F18 — LOW — the P4c transcript-status list is stale

Status: OPEN.
Plan sections: 0, clause ownership; 6.1, generated lists.

Plan evidence at the plan commit above:
> "generated by `docs/stage1-clauses/owners.py` from the ledger at the botster-contracts commit above"

List evidence at the same plan commit, `docs/stage1-clauses/p4c-webrtc-perf.txt`:
> "conf::dp_3_datachannel_chunking_matches_tr3\tDP-3\tpending"

At the pinned contracts commit, `conformance/pending.txt` lists none of P4c's 15 ids.
The ownership generator produces `transcript` for every P4c row, while the checked-in list still says `pending` for all 15.
Every other generated list matches the pinned contracts revision.

Required change: regenerate the P4c list against `contracts-v0.1.9` and commit the 15 transcript-status changes.

VERDICT: NOT CLEAN (2 open)

## Round 19 — revision 19

Plan: `f8c9ade30c3e8437160db0c05551a93ee8191f1b`, `docs/stage1-plan.md`.
Pin: `stage1-plan.62f664de.md`.
Verified SHA-256: `62f664de2476bc172336a9ddac7b111517a9fc49f34eb59b69a8d11a24c0bea8`.
Contracts tag: `contracts-v0.1.9`, commit `7f72acf8427ad7bf414db42d2360d5dccc7b3e13`, manifest final22.
Ghostty fork: `85a8d8eb197c5752887c017c9a3faa6f1dc1969b`.

I reviewed the complete delta and F17 and F18.
The plan matches its pin.
F17 is CLOSED: model steps use completed calls and consumed prefixes, with the unconsumed suffix retained in order.
Section 2.4 now includes query-capacity backpressure, final-mode comparison after each step, and input admission only between completed steps.
The architecture still prohibits terminal parsing outside libghostty.
F18 is CLOSED: all 15 P4c rows now match the transcript status at the pinned contracts tag.
All other ownership lists are unchanged from the reviewed revision 18.
F1 through F16 remain CLOSED. All eighteen findings are closed.
This verdict covers the plan at the commit above.
It does not approve product code or establish Stage 1 acceptance.
I ran no product tests.

VERDICT: CLEAN
