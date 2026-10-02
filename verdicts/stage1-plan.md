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
