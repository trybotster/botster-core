# P5 contracts v0.1.21 pin — PR #205 integration review

## Round 1 — 2026-10-09 — CLEAN

Reviewed head: `dd2002e51390a812401d6d3d0a21575cb60474f9`.
Base: `7e35566614bc61140ec254331db48ed7f568f710`.
Contracts pin: `contracts-v0.1.21`, commit `60a4169978e3f704f46ab0578f9993013fd4b810`.
Previous pin: `contracts-v0.1.20`, commit `03891658e793e5400ba46b5bc003b5d9f952f5e2`.
Tier: HIGH, rule 4, because this changes a contract pin.
The reviewer read the complete four-file delta, PR description, Core contract and conformance changes, manifest, and supplied gate.
The reviewer changed no product code and ran no builds, tests, mutants, or gates.

### Pin and contract scope

All six direct contracts dependencies use the new tag.
Every contracts package in Cargo.lock resolves to the same full commit, which matches the local tag target.
The additional hub-contract dependency on `data-encoding` uses the package already present in the lockfile.
No unrelated package version changes.

Manifest final36 includes Core A18 candidate 2, HC A7 candidate 2, and UI erratum 2 candidate 3.
The reviewer read the complete frozen A18 text and checked its hash against the manifest:
`6fcd07624da4dbc24d6c07aea29526443390d28219cce9bbe9b1c5e2f8edafeb`.
A18 defines `last_output_at` as this host's output observation and defines the adopted idle start at the adoption event.
It retains the monotonic threshold and E3-1 event-budget rule.

The AD-3 transcript now excludes only `last_output_at` from both terminal-state objects before comparing them.
The new `let without` operation copies an object and removes the named field, with object/name validation.
The upstream tests cover a present field, an absent field, and invalid argument types.
AD-3 remains pending in Core. This pin does not claim the transcript now passes or implement A18.

The Core contract and route codec add `CORE_SERVICE_TRANSPORT_UNIX_LANES_1` and `ROUTE_FEATURES` to their preludes.
No Core source or test declares either name. The supplied compilation and public-API checks pass with the new pin.
The broader HC and UI crate additions remain upstream contract changes; this PR changes no Core Rust source.

### Ledger and merge

The checked-in Core ledger exactly equals the new pin's Core ledger: 679 IDs, up from 675.
Exactly four IDs are added, all from A18. No old ledger ID is removed.
The pending list adds exactly those four IDs, and the new pin marks each pending without a transcript.
This imports new contract requirements. It does not move an existing passing ID back to pending.
The two A15 comments correctly name the new pin; their status is unchanged.
The deferred and withdrawn copies exactly equal the new pin's files and remain unchanged.

The merge parents are pin commit `8debbea28d31746c9efd7f6439a9dbbe1ce7e5c8` and the base above.
The resolved pending list preserves #204's removals and adds only the four new A18 IDs against that base.
The complete merge delta contains only Cargo.toml, Cargo.lock, the ledger copy, and the pending list.
The base is the fetched v1 tip and is an ancestor of the head. `git diff --check` passes.
The approved minimum list remains 70 IDs, with 39 active testkit IDs before and after this pin.
No real-process gain is claimed.

### Supplied evidence

Gate:
`~/botster-sessions/gates/botster-core-stage1-p5-pin-v0.1.21-dd2002e5-pool-20261009-162431-34685.log`.
The log names the exact reviewed head and base. Linux node `msa1` used allocation `0c63ff48`.
All ten full CI checks passed. The ledger check confirms 679 IDs and matching status files.
The default tier passed 1170 tests. The slow tier passed 249 tests. Conformance reports 113 passed and zero failed.
The conformance count matches the base's count. The four new requirements remain pending.
The full gate and separate slow-profile mutation job both report no Rust source change and no mutation outcomes.
Those reports provide no new mutation proof, including for upstream crate changes.
The gate exited zero after 53 seconds.

No integration finding remains for this pin move. A18 implementation and transcript activation remain separate work.

VERDICT: CLEAN (0 open) at dd2002e51390a812401d6d3d0a21575cb60474f9
