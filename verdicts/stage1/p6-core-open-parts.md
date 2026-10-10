# Production composition export integration review

## Round 1 — PR #209

Reviewed head: `79107e9acbf175baccc4f782fded2c435aefb680`.
Base: `b453b449a959f7e10b94733b4425b4903d6a4890`, the fetched v1 head and an ancestor of the reviewed head.
Tier: HIGH for the facade API and shared production composition.
Authority: approved plan 23l at `a538765fe9e3a8243bad5acf2a44619510c34edc`.
Scope: all three changed files, the full PR body, the constructor and resource ownership, and the exact-head gate.

The facade exports RealEdges and open_parts. Core::open calls open_parts, then HostDriver::open.
The extracted composition preserves the previous checks, error mapping, resource creation, features, protocol, and terminal identity.
A token comparison confirms that the moved body is unchanged after whitespace removal and the RealEdges name qualification change.
The private RealEdges fields retain ownership of the data-directory lock, listener, streams, and child processes.
RealEdges::new is crate-visible; external callers construct the production edges through open_parts.
The change adds no Core injection constructor, feature flag, or test branch.

The API snapshot adds RealEdges, its exposed trait methods and auto traits, and open_parts. It removes nothing.
The public signatures name botster-core-host and botster-core-edges types, as the PR explicitly states.
Those component crates retain the plan's existing compatibility scope. The approved composition export requires those types.

This PR supplies the shared production composition only.
The RealCoreHarness PR must still prove bounded pass-through behavior and closed-LinkId behavior under the approved plan.
This review does not claim that either wrapper proof or any new real-tier transcript passes.

Gate: `/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-p6-core-open-parts-79107e9a-pool-20261009-173007-35159.log`.
The log names this exact head and base. It ran on msa1, allocation 36bc9532, and exited zero after 422 seconds.
All ten gate stages pass, including the regenerated public API check.
Default: 1277 passed. Slow: 254 passed. Conformance: 121 passed, zero failed.
The real-process tests cover configuration checks, directory privacy, exclusive open, and the wake descriptor through Core::open.
Mutations: two tested, both unviable, zero caught, zero missed, and zero timeouts.
The unmutated baseline passes; this result provides no caught-mutant claim for the extraction.
No pending ID changes, mutation exclusions, or test changes occur in this delta.
`git diff --check` passes. The reviewer ran no builds, tests, or gates.

No integration finding remains open. The separate package review remains required for merge.

VERDICT: CLEAN at 79107e9acbf175baccc4f782fded2c435aefb680

## Round 2 — Documentation clarification and base refresh

Reviewed head: `b89c850e1f174193a39322aafb07784b2f3b8852`.
Current base: `5348befac55b1b9a5d7d21f14ae77f6b123515c3`.
Scope: the complete delta from round 1 head `79107e9acbf175baccc4f782fded2c435aefb680`.

The documentation head `329720e14dcb53779ecf6218c8f8b4e18f0c08fe` changes only two documentation passages.
The RealEdges export states that TH-1's Send, not Sync property belongs to Core, not to the edges.
The open_parts documentation requires callers to pass the returned EngineConfig to HostDriver::open unchanged.
Both statements clarify the approved composition. They change no code or API signature.
The documentation delta has no finding.

The final head merges current v1 onto that documentation head.
All three PR files remain byte-identical to the documentation head.
The five imported files equal current v1 byte for byte and contain the already reviewed #184 union.
The base-merge check reports every check PASS and exit zero.
It confirms no conflict, no shared changed path, an identical 9251-byte PR diff, and an unchanged pending list.
The diff hash is `b54a99160d0e427a68613eb26d7531a6a8008b6bd1f2d130fe2ee7213a715c60`.

Gate: `/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-p6-core-open-parts-b89c850e-pool-20261009-174454-92278.log`.
The log names the reviewed head and current base. It ran on msa1, allocation 138933a1, and exited zero after 143 seconds.
All ten full gate stages pass. Default: 1285 passed. Slow: 254 passed. Conformance: 121 passed, zero failed.
Two mutants are unviable; zero are caught, missed, or timed out. The unmutated baseline passes.
This result makes no caught-mutant claim for the extraction.
`git diff --check` passes. The reviewer ran no builds, tests, or gates.

Round 1's wrapper and closed-LinkId proof obligations remain assigned to the RealCoreHarness PR.
No integration finding remains open. The separate package verdict remains required for merge.

VERDICT: CLEAN at b89c850e1f174193a39322aafb07784b2f3b8852
