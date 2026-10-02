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
