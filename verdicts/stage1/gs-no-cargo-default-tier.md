# Core #310: integration review

## Round 1

- Reviewer: Sol-B.
- Lead assignment: `msg_plugin-w_1791818404_e2c385`; #310 precedes #313.
- Branch: `stage1/gs-no-cargo-default-tier`.
- Exact head: `204e91d820c2d719c00b3665e2d5ba6aa436b0d0`.
- Tree: `f666f591ac1b4b97b99a9b77b1ecad8ff59a8260`.
- Source and gate base: `be95ef13edc6d9eccf87a8c0b2c09b9cf820963a`.
- Risk: HIGH; CI tooling.

Sol-B read the complete four-file delta, the full body, affected callers, and test-tier selection.
The delta has 245 additions and eighteen deletions.
It changes no product dependency, candidate binary, conformance status, mutation exclusion, timeout, or budget.

### Source and integration

The default job-common test no longer calls Cargo metadata.
It reads the pinned contracts source from Cargo.lock and requires one matching Cargo checkout path.
The pure resolver requires the repository prefix and a hexadecimal revision prefix of at least seven characters.
It rejects no match and multiple matches. The wrapper reads files and starts no process.
The test retains the nonempty-copy assertion and comparison with the pinned job-common text.

The resolver uses checkout directory names. It does not independently verify the checkout's Git content.
The new slow test compares its canonical path with Cargo metadata.
The production lists command still obtains the contracts path from Cargo metadata and compares the checked-in copy.
Thus the file-only default test does not replace the production gate check.
The test-only functions have no production caller.

The packages_of extraction retains the earlier package and target parsing.
The production packages caller still executes Cargo metadata and canonicalizes the root before parsing.
The default fixture checks features, target kinds, names, relative paths, required features, and exclusion outside the root.
The real workspace metadata test moves unchanged into mutants_cited::slow_tests.
Both added or moved real checks match the existing slow-module filter.

Sol-B inspected the named Cargo callers and their conditional compilation boundaries.
The remaining Cargo execution occurs in production commands and slow-test modules.
The default caps test constructs a Cargo command and reads its environment; it does not execute the command.
The changed default tests start no Cargo process.
This is a source caller audit. The PR adds no mechanical rule against a future default test that starts Cargo.

### Earlier failure and completed evidence

Sol-B read the failed #306 re-gate's timeout and leftover-process records.
Log: `/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-gs-built-in-coverage-2fa58b79-pool-20261011-224037-80412.log`.
The job-common default test times out at 2.002 seconds. The log reports an orphaned Cargo metadata process.
The test-budget step fails. Slow, mutants, and fuzz do not run. The wrapper exits 1 after 274 seconds.
That gate remains failed. Source inspection establishes that the old test starts Cargo; lock contention remains the accepted diagnosis.
Sol-B ran no reproduction and does not claim that the log directly measures a lock wait.

Completed #310 log: `/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-gs-no-cargo-default-tier-204e91d8-pool-20261011-230511-22291.log`.
SHA256: `f199169097c9993c2e1f5d532d2bdc5ac659e70bef7aadf2f7889242024ddf10`.
Sol-B read the exact-head header, base accounting, changed-test results, raw mutation records, citations, and final exits.
The lists step records the same source, gate, and merge base.

All ten steps PASS in 481.4 seconds on msa1.
Default passes 1,921 tests. The job-common test takes 0.006 seconds; the fixture parser test takes 0.002 seconds.
Slow passes 619 tests. The resolver comparison takes 0.114 seconds; the moved metadata test takes 0.017 seconds.
The unmutated slow baseline passes all 619 tests.

Stage 1 tests four mutants: one caught, one missed, and two unviable.
The default fixture catches the empty packages_of replacement.
Stage 2 catches the empty packages replacement through the moved real metadata test.
Sol-B independently matches both raw catch identities to their citations. Each catcher has a normal PASS.
The final result has two caught, two unviable, zero missed, and zero timeout. Unviable outcomes supply no catch evidence.
The test-only resolver receives no cargo-mutants mutant. Its fixture and real-path comparison supply its recorded test evidence.
No FLAKY record appears. Fuzz reports no changed decoder-harness crate.
The job and wrapper exit 0 after 680 seconds, with 119 seconds queued and 561 seconds running.

The body names the caller audit, changed tests, exact head/base, and these completed gate results.
The local Mac results remain author reports. Sol-B claims no independent local green result.
Sol-B read package verdict `7565413918c9ed0c31c50ba6ae27d26d5f6f2550`, `verdicts/gs-gate-speed.md`, both #310 sections.
Its exact-head source and evidence conclusions agree with this review. No package finding remains.

### Disposition

No source, body, or supplied-evidence finding remains on this exact head.
This CLEAN accepts the reviewed head and completed gate. It accepts no later import, train, or contracts pin.
The lead owns the authorized final import or train procedure and merge verification.
This verdict authorizes no retry of #306 and does not accept its failed gate.
Sol-B ran no build, test, gate, mutation, reversal, diagnostic, or executable base check.

VERDICT: CLEAN (0 open)

## Round 2: train 3c source delta

Exact train head: `b78df725177d1a931e1eddb9bb67023e7e0ae645`.
Tree: `43cdf59980d93e0672423bedc0edf506405256be`.
Base: `080987829a2e2245a7ea97606020400392c974d6`.
Frozen #310 member: `204e91d820c2d719c00b3665e2d5ba6aa436b0d0`.
The train merges #310 first, then #312's .44 pin. It contains neither #308 nor #309.

Sol-B independently compares all four own patches with the first train merge.
Every added and deleted line matches. All four file blobs equal the accepted member blobs at the final train head.
The .44 lock source retains the repository and package identity used by the resolver.
The resolver follows the exact delivered lock revision. Its .43 fixture remains a synthetic input only.
The slow canonical-path comparison remains selected. The pure packages parser and real metadata test retain their reviewed behavior.
No default test adds a Cargo launch. No timing allowance, gate verdict, mutation exclusion, or own pending removal changes.

Sol-B read exact-head package delta `28212287cb8cae87306e46f2ae58d235b113e640`, train 3c section.
It agrees with these source checks. No #310 source delta finding remains.
The combined source review is Round 2 in `verdicts/stage1/train-3.md` in this commit.
The original evidence retains its original head. The final train gate and member evidence receipt remain due.
This verdict accepts no later head or #306 retry. Sol-B ran no execution job.

VERDICT: CLEAN (0 open; source delta only, final train evidence pending)
