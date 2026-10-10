# Contracts v0.1.22 pin — PR #211

## Round 1 — CLEAN

Reviewed head: `19e918071660a12a5a087960a45d9530d1a0ab7b`.
Current v1 base: `4b227d46ef5453ab8a3f7daaa4dc33562b4be83b`.
Risk: HIGH, rule 4. Scope: the full eighteen-file delta and its dependency and conformance effects.

### Pin and source

The six workspace dependencies select contracts-v0.1.22.
The tag resolves to `af5771cf962eb26b7074d486dafbb3360ff50d01`, the revision selected by approved plan 23m.
The nine contracts packages in Cargo.lock change only their source fields to that tag and commit.
Package identities, dependency lists, checksums, and versions remain unchanged.

The reviewer read the relevant contracts crate changes and Core A17 candidate 2.
Core A17 removes the WebRTC operation, transport, features, errors, options, and limits.
The Core admission and refusal changes remove the corresponding uses.
The transport check remains because RouteTransport is non-exhaustive.
The test changes remove obsolete cases and fields. The two updated count tests match the new tag.
The contract prelude removes Answer and OfferRefusal without adding a name that could collide with local code.
The codec removes the route-level max_chunk_bytes field. Core source retains no use of that field or the deleted options.
The conformance driver substitutes bound variables in attach options and removes the retired operation handling.
The runner adds Limits::real. This PR does not use that limit or change a Core timeout.
No mutation exclusion changes occur.

### Conformance accounting

Read-only comparisons independently establish these facts:

- The checked ledger contains exactly the tag's 682 Core IDs.
- The withdrawn and deferred files match the tag byte for byte.
- All 15 IDs removed from pending are newly withdrawn Core IDs. All were pending at the base.
- The three pending additions are exactly the new A17 ledger IDs. The tag also lists them as pending without transcripts.
- Pending changes from 554 to 542. All 69 minimum IDs remain in scope.
- The gate's 121 passing conformance IDs exactly equal the passing set from the #209 gate on the base tree.

The contracts delta modifies 21 Core transcripts and deletes the 15 withdrawn transcripts.
The reviewer read the four modified transcripts that already pass on v1.
Three wait for output after ignore_sigterm before testing Stop behavior.
The zero-limit transcript removes the three retired limits.
All four have PASS evidence at the reviewed head.
This PR changes no transcript and claims no real-harness completion.

### R1-1 — LOW — Transcript count corrected within this round

The original PR body said 19 Core transcripts changed, although its lists named all 21.
The reviewer confirmed 21 modified and 15 deleted files in the tag delta and sent the correction to P6.
P6 changed the count to 21 without changing the head.
The reviewer read the corrected GitHub body. This finding is closed.
The package review records the same finding as C22-F1 and closes it.

### Evidence and acceptance

Gate: `/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-p6-pin-v0.1.22-19e91807-pool-20261009-181249-20493.log`.
The log names the exact head and current base. The head contains that base.
The Linux pool job ran on msa1, allocation 520333d8. The gate exits zero after 198 seconds.
All ten stages report PASS. Fuzz reports no changed crate with a decoder harness.
Default: 1284 passed. Slow: 254 passed.
Conformance: 121 passed, zero failed, 478 pending with transcripts, 64 pending without transcripts, two deferred, and 17 withdrawn.
Both mutation invocations report six mutants: four caught, two unviable, zero missed, and zero timeouts.
The extra environment-only invocation does not establish slow-feature mutation coverage.
There is no new slow-evidence exclusion in this PR. The separate gate correction belongs to plan 23n.

The reviewer read package verdict `795d1376448666f5bf25c59c9cf844296bc3847c`, which is CLEAN on this exact head.
The final remote check confirms that the head and v1 base remain unchanged.
`git diff --check` passes. The reviewer ran no builds, tests, or gates.

The later merge with #206 must remove its five lines that use fields deleted by A17.
A merge that changes the reviewed diff requires a delta review and a gate on the resulting head.
The separate #210 findings remain open. This verdict closes no pending proof obligation.
No finding remains open for #211.

VERDICT: CLEAN at 19e918071660a12a5a087960a45d9530d1a0ab7b
