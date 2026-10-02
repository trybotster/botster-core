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
