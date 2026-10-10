# P1 package review — RealEdges and open_parts

## PR #209 — Round 1 — 2026-10-09

- Exact head: `b89c850e1f174193a39322aafb07784b2f3b8852`.
- Tree: `b372a84c84688aaa01ebc25c774fc62ca0c5fcb0`.
- Parents: `329720e14dcb53779ecf6218c8f8b4e18f0c08fe` and `5348befac55b1b9a5d7d21f14ae77f6b123515c3`.
- PR and gate base: `5348befac55b1b9a5d7d21f14ae77f6b123515c3`.
- Risk: HIGH. This PR changes the public facade API. The lead assigned this P1 package review to P5.
- Approved plan: revision 23l at `a538765f`, pin `~/botster-sessions/pins/stage1-plan.a538765f.md`.
  Pin SHA-256: `f686ecff0691cb9c25da0acfbf68e70fedc78e7c8627f6de6535f8103215d7b6`.

The reviewer read the complete three-file PR delta, the documentation follow-up, and the base merge.
The PR explains its prior art: it moves the existing `Core::open` setup and uses the existing `HostDriver` composition.
No package source finding remains open.

### Source and API review

`open_parts` performs the existing config and socket checks, opens `DataDir`, creates `RealEdges`, and builds `EngineConfig`.
The reviewer compared the moved body with the old `Core::open` body.
The bodies match after whitespace and `RealEdges` name qualification are normalized.
Error mapping, features, terminal identity, limits, host epoch, worker path, and worker protocol remain unchanged.
`Core::open` calls `open_parts`, then `HostDriver::open`, and retains its `PhantomData<Cell<()>>` field.

`RealEdges` fields remain private. Its constructor is `pub(crate)`.
`open_parts` is the public constructor for these parts. The PR adds no Core injection constructor or feature flag.
Construction, directory lock ownership, listener ownership, wake handling, reaper ownership, and Drop behavior remain unchanged.

The API snapshot retains its existing prefix and adds 31 lines.
The additions expose `RealEdges`, its `HostEdges` methods and trait implementations, and `open_parts`.
The snapshot does not expose `RealEdges::new`.
The PR states that this composition uses types from internal crates without a compatibility promise, as plan section 2.6 permits.
Documentation states that TH-1 applies to the `Core` handle and that callers pass `EngineConfig` unchanged.

The three-file PR delta is byte-identical before and after the base merge.
The five imported file changes equal the v1 delta. The merge adds no change to botster-core source.
Pending IDs remain unchanged. All 41 active minimum IDs have PASS evidence; the minimum count remains 41 / 69.
The PR changes no contracts pin, transcript, timeout value, or mutation exclusion.

### Finding closure

**OP-F1 — LOW — CLOSED within Round 1.**
The PR body initially called `79107e9a` the exact head and cited only that head's gate.
The reviewer sent this finding directly to P6 and Astra.
P6 changed the body without changing the head.
The reviewer read the corrected body through GitHub and verified the exact head remained `b89c850e1f174193a39322aafb07784b2f3b8852`.
The body now names the current head, v1 base, passing base-merge check, and current full gate.
The body retains the old gate as earlier evidence and states the head history.
No finding remains open, including LOW findings.

### Supplied execution evidence

The reviewer read:
`~/botster-sessions/gates/botster-core-stage1-p6-core-open-parts-b89c850e-pool-20261009-174454-92278.log`.

The log names the exact reviewed head and base.
The base-merge check reports PASS for every check and `BASE_MERGE_CHECK_EXIT=0`.
Its own-diff comparison reports 9251 bytes with SHA-256
`b54a99160d0e427a68613eb26d7531a6a8008b6bd1f2d130fe2ee7213a715c60`.
All ten full-gate stages report PASS. The job and gate exit with zero.

- Default tests: 1285 passed, 558 skipped.
- Slow tests: 254 passed, 1273 skipped.
- Conformance: 121 passed, zero failed.
- Mutants: two tested, zero caught, two unviable, zero missed, zero timeout. The baseline passed.
- Fuzz: no changed decoder crate.

The reviewer makes no mutation-kill claim from the two unviable mutants.
Existing facade forwarding and real worker-link tests have PASS lines.
The open-config and directory-exclusivity tests also have PASS lines.
The reviewer ran no builds, tests, base-merge checks, mutation jobs, or gates.
The reviewer changed no product code.

### Scope and integration

The harness must supply the pass-through wrapper proof and the closed-LinkId proof in its later PR.
This verdict accepts the export and shared composition. It does not approve unwritten wrapper behavior.
Astra separately reports integration CLEAN on this exact head in verdict commit
`a5d9d2bed05e7e670cceb307f420bc78c8135246`, `verdicts/stage1/p6-core-open-parts.md`, Round 2.
The corrected body closes the package finding after that integration source and gate review.

VERDICT: CLEAN
