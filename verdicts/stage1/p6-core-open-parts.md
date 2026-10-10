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
